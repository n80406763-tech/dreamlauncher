//! Оркестрация установки: от "версия + загрузчик" до полностью
//! скачанных client.jar/библиотек/натив-либ/ассетов/Java, готовых к
//! `launch`. Каждый шаг сам по себе уже реализован (`meta`, `download`,
//! `java`, `loaders`) — этот модуль просто их последовательно вызывает.

use crate::download::{self, DownloadEvent, DownloadTask, DownloaderConfig};
use crate::error::{CoreError, Result};
use crate::java::{self, RuntimeFilesManifest};
use crate::launch::{self, AccountKind, LaunchCommand, LaunchContext, QuickPlay};
use crate::loaders::fabric_like::{fetch_profile_json, FabricLikeKind};
use crate::loaders::forge::{self, ForgeInstallContext, InstallProfile};
use crate::loaders::LoaderKind;
use crate::meta::rules::rules_allow;
use crate::meta::{self, AssetIndex, AssetLayout, RawVersionJson, ResolvedVersion, RuleContext};
use crate::storage::AppPaths;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct VersionRef {
    pub mc_version: String,
    pub loader: LoaderKind,
    /// Обязателен для Fabric/Quilt; для Forge/NeoForge/Vanilla не используется.
    pub loader_version: Option<String>,
}

/// Имя каталога версии в `shared/versions/` — для ванили просто id версии,
/// для загрузчиков дописывается его имя и версия, чтобы разные загрузчики
/// на одной версии игры не делили один и тот же classpath/натив-каталог.
pub fn version_dir_name(v: &VersionRef) -> String {
    match v.loader {
        LoaderKind::Vanilla => v.mc_version.clone(),
        other => format!("{}-{}-{}", v.mc_version, other.as_str(), v.loader_version.as_deref().unwrap_or("unknown")),
    }
}

#[derive(Debug, Clone)]
pub enum InstallEvent {
    Phase(&'static str),
    Download(DownloadEvent),
}

/// Данные, нужные только для Forge/NeoForge — запуск процессоров
/// патчинга после того, как все обычные файлы уже на диске.
struct ForgePlanExtra {
    install_profile: InstallProfile,
    installer_path: PathBuf,
}

/// Всё, что нужно скачать/распаковать, посчитано заранее — отдельно от
/// исполнения, чтобы можно было показать пользователю объём работы до
/// того, как начнётся сеть.
pub struct InstallPlan {
    pub resolved: ResolvedVersion,
    pub loader: LoaderKind,
    pub version_dir: PathBuf,
    pub client_jar: PathBuf,
    pub natives_dir: PathBuf,
    pub java_home: PathBuf,
    pub java_already_installed: bool,
    download_tasks: Vec<DownloadTask>,
    /// (путь к jar-натива в shared/libraries, префиксы для исключения из распаковки)
    native_jars: Vec<(PathBuf, Vec<String>)>,
    asset_index_path: PathBuf,
    assets_objects_dir: PathBuf,
    assets_dir: PathBuf,
    libraries_dir: PathBuf,
    forge: Option<ForgePlanExtra>,
}

impl InstallPlan {
    pub fn total_downloads(&self) -> usize {
        self.download_tasks.len()
    }
}

/// Загружает и сливает цепочку version JSON для ванили либо Fabric/Quilt.
/// Для Forge/NeoForge используется `resolve_forge_like` — там помимо
/// `ResolvedVersion` нужен ещё и разобранный `install_profile.json`.
pub async fn resolve_version(client: &reqwest::Client, v: &VersionRef) -> Result<ResolvedVersion> {
    let manifest = meta::fetch_version_manifest(client).await?;
    let entry = manifest.find(&v.mc_version).ok_or_else(|| CoreError::VersionNotFound(v.mc_version.clone()))?;
    let vanilla_json = meta::fetch_version_json(client, &entry.url).await?;

    let chain = match v.loader {
        LoaderKind::Vanilla => vec![vanilla_json],
        LoaderKind::Fabric | LoaderKind::Quilt => {
            let kind = if v.loader == LoaderKind::Fabric { FabricLikeKind::Fabric } else { FabricLikeKind::Quilt };
            let loader_version = v.loader_version.as_deref().ok_or_else(|| CoreError::Other("не указана версия загрузчика".into()))?;
            let loader_json = fetch_profile_json(client, kind, &v.mc_version, loader_version).await?;
            vec![loader_json, vanilla_json]
        }
        LoaderKind::Forge | LoaderKind::NeoForge => {
            return Err(CoreError::Other("для Forge/NeoForge используйте resolve_forge_like".into()));
        }
    };

    meta::merge_chain(chain)
}

/// То же самое для Forge/NeoForge: качает (или берёт из кэша)
/// installer-jar, достаёт из него `install_profile.json`/`version.json` и
/// сливает `version.json` с ванильным родителем — формат идентичен
/// Fabric/Quilt (`inheritsFrom`), NeoForge унаследовал его от Forge.
pub async fn resolve_forge_like(client: &reqwest::Client, paths: &AppPaths, v: &VersionRef) -> Result<(ResolvedVersion, InstallProfile, PathBuf)> {
    let manifest = meta::fetch_version_manifest(client).await?;
    let entry = manifest.find(&v.mc_version).ok_or_else(|| CoreError::VersionNotFound(v.mc_version.clone()))?;
    let vanilla_json = meta::fetch_version_json(client, &entry.url).await?;

    let loader_version = v.loader_version.as_deref().ok_or_else(|| CoreError::Other("не указана версия загрузчика".into()))?;
    let installer_url = match v.loader {
        LoaderKind::Forge => forge::forge_installer_url(&v.mc_version, loader_version),
        LoaderKind::NeoForge => forge::neoforge_installer_url(loader_version),
        _ => return Err(CoreError::Other("resolve_forge_like вызван для не-Forge загрузчика".into())),
    };

    // Installer кэшируется на диске: он же нужен на шаге процессоров как
    // реальный файл (некоторые процессоры сами открывают его как zip
    // через `{INSTALLER}`), а не только как источник JSON здесь.
    let cache_dir = paths.cache_dir().join("installers");
    std::fs::create_dir_all(&cache_dir).map_err(|e| CoreError::io(cache_dir.display().to_string(), e))?;
    let installer_path = cache_dir.join(format!("{}-{}-{loader_version}-installer.jar", v.loader.as_str(), v.mc_version));

    if !installer_path.is_file() {
        let bytes = client.get(&installer_url).send().await?.error_for_status()?.bytes().await?;
        let tmp = installer_path.with_extension("jar.part");
        std::fs::write(&tmp, &bytes).map_err(|e| CoreError::io(tmp.display().to_string(), e))?;
        std::fs::rename(&tmp, &installer_path).map_err(|e| CoreError::io(installer_path.display().to_string(), e))?;
    }

    let installer_bytes = std::fs::read(&installer_path).map_err(|e| CoreError::io(installer_path.display().to_string(), e))?;
    let profile_text = forge::read_zip_entry_text(&installer_bytes, "install_profile.json")?;
    let profile = InstallProfile::parse(&profile_text)?;
    let version_text = forge::read_zip_entry_text(&installer_bytes, "version.json")?;
    let forge_version_json: RawVersionJson = serde_json::from_str(&version_text).map_err(|e| CoreError::json("forge/neoforge version.json", e))?;

    let resolved = meta::merge_chain(vec![forge_version_json, vanilla_json])?;
    Ok((resolved, profile, installer_path))
}

/// Считает полный план установки: резолвит версию, скачивает и разбирает
/// asset index, при необходимости — манифест Java-рантайма. Сетевые
/// запросы здесь — только за метаданными (JSON), не за самими файлами.
pub async fn plan_install(client: &reqwest::Client, paths: &AppPaths, v: &VersionRef) -> Result<InstallPlan> {
    let (resolved, forge_extra) = match v.loader {
        LoaderKind::Forge | LoaderKind::NeoForge => {
            let (resolved, profile, installer_path) = resolve_forge_like(client, paths, v).await?;
            (resolved, Some(ForgePlanExtra { install_profile: profile, installer_path }))
        }
        _ => (resolve_version(client, v).await?, None),
    };
    let ctx = RuleContext::current();

    let dir_name = version_dir_name(v);
    let version_dir = paths.versions_dir().join(&dir_name);
    let client_jar = version_dir.join(format!("{dir_name}.jar"));
    let natives_dir = version_dir.join(format!("natives-{}-{}", ctx.os_name, ctx.os_arch));
    let libraries_dir = paths.libraries_dir();

    let mut tasks = Vec::new();

    let client_dl = resolved.downloads.client.as_ref().ok_or_else(|| CoreError::Other(format!("версия {} не содержит downloads.client", v.mc_version)))?;
    tasks.push(DownloadTask { url: client_dl.url.clone(), dest: client_jar.clone(), sha1: Some(client_dl.sha1.clone()), size: Some(client_dl.size) });

    let mut native_jars = Vec::new();
    for lib in resolved.applicable_libraries(&ctx) {
        if let Some(artifact) = lib.main_artifact() {
            tasks.push(DownloadTask {
                url: artifact.url,
                dest: libraries_dir.join(artifact.relative_path.replace('/', std::path::MAIN_SEPARATOR_STR)),
                sha1: artifact.sha1,
                size: artifact.size,
            });
        }
        if let Some(native) = lib.native_artifact(&ctx) {
            let dest = libraries_dir.join(native.relative_path.replace('/', std::path::MAIN_SEPARATOR_STR));
            tasks.push(DownloadTask { url: native.url, dest: dest.clone(), sha1: native.sha1, size: native.size });
            let exclude = lib.extract.as_ref().map(|e| e.exclude.clone()).unwrap_or_default();
            native_jars.push((dest, exclude));
        }
    }

    let assets_dir = paths.assets_dir();
    let assets_objects_dir = paths.assets_objects_dir();
    let asset_index_path = assets_dir.join("indexes").join(format!("{}.json", resolved.asset_index.id));
    tasks.push(DownloadTask {
        url: resolved.asset_index.url.clone(),
        dest: asset_index_path.clone(),
        sha1: Some(resolved.asset_index.sha1.clone()),
        size: Some(resolved.asset_index.size),
    });

    // Индекс ассетов нужен прямо сейчас (не после скачивания), чтобы
    // расписать объекты как часть того же плана — качаем отдельным
    // запросом (это тот же файл, что уйдёт в tasks выше; при исполнении
    // он будет пропущен как уже валидный, если уже на диске).
    let asset_index_text = client.get(&resolved.asset_index.url).send().await?.error_for_status()?.text().await?;
    let asset_index = AssetIndex::parse(&asset_index_text)?;
    tasks.extend(asset_index.download_tasks(&assets_objects_dir));

    // Библиотеки-инструменты процессоров патчинга (installertools,
    // jarsplitter, ForgeAutoRenamingTool, binarypatcher, mcp_config и
    // т.д.) — их нет в `resolved.libraries` (та версия игры в них не
    // нуждается), они нужны только на время установки.
    if let Some(extra) = &forge_extra {
        for lib in &extra.install_profile.libraries {
            if !rules_allow(&lib.rules, &ctx) {
                continue;
            }
            if let Some(artifact) = lib.main_artifact() {
                tasks.push(DownloadTask {
                    url: artifact.url,
                    dest: libraries_dir.join(artifact.relative_path.replace('/', std::path::MAIN_SEPARATOR_STR)),
                    sha1: artifact.sha1,
                    size: artifact.size,
                });
            }
        }
    }

    let (java_component, _) = java::required_component(&resolved);
    let java_home = paths.java_dir().join(java_component);
    let java_already_installed = java::is_installed(&java_home);
    if !java_already_installed {
        let runtime_manifest = java::fetch_java_runtime_manifest(client).await?;
        let entry = runtime_manifest.entry(java::current_platform_key(), java_component).ok_or_else(|| CoreError::JavaNotFound { component: java_component.to_string() })?;
        let files_text = client.get(&entry.manifest.url).send().await?.error_for_status()?.text().await?;
        let files_manifest = RuntimeFilesManifest::parse(&files_text)?;
        tasks.extend(files_manifest.download_tasks(&java_home));
    }

    Ok(InstallPlan {
        resolved,
        loader: v.loader,
        version_dir,
        client_jar,
        natives_dir,
        java_home,
        java_already_installed,
        download_tasks: tasks,
        native_jars,
        asset_index_path,
        assets_objects_dir,
        assets_dir,
        libraries_dir,
        forge: forge_extra,
    })
}

/// Выполняет план: создаёт каталоги, качает всё одним пакетом, затем
/// распаковывает нативы и (для старых версий) раскладывает ассеты в
/// `virtual/legacy`.
pub async fn execute_install(client: reqwest::Client, plan: &InstallPlan, events: mpsc::UnboundedSender<InstallEvent>) -> Result<()> {
    std::fs::create_dir_all(&plan.version_dir).map_err(|e| CoreError::io(plan.version_dir.display().to_string(), e))?;
    std::fs::create_dir_all(&plan.natives_dir).map_err(|e| CoreError::io(plan.natives_dir.display().to_string(), e))?;
    std::fs::create_dir_all(&plan.assets_objects_dir).map_err(|e| CoreError::io(plan.assets_objects_dir.display().to_string(), e))?;
    if let Some(parent) = plan.asset_index_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CoreError::io(parent.display().to_string(), e))?;
    }
    // Каталог самой Java отдельно создавать не нужно: движок загрузки
    // сам делает `create_dir_all` для родителя каждого файла перед
    // записью (см. `download::engine::download_once`) — этого достаточно
    // для всех файлов рантайма, библиотек и ассетов.

    let _ = events.send(InstallEvent::Phase("Загрузка файлов"));
    let (dl_tx, mut dl_rx) = mpsc::unbounded_channel();
    let forward = {
        let events = events.clone();
        tokio::spawn(async move {
            while let Some(ev) = dl_rx.recv().await {
                if events.send(InstallEvent::Download(ev)).is_err() {
                    break;
                }
            }
        })
    };
    download::download_all(client, plan.download_tasks.clone(), DownloaderConfig::default(), dl_tx).await?;
    let _ = forward.await;

    let _ = events.send(InstallEvent::Phase("Распаковка нативных библиотек"));
    for (jar_path, exclude) in &plan.native_jars {
        let bytes = std::fs::read(jar_path).map_err(|e| CoreError::io(jar_path.display().to_string(), e))?;
        download::safe_extract(&bytes, &plan.natives_dir, &download::ExtractOptions { exclude_prefixes: exclude })?;
    }

    let asset_index_text = std::fs::read_to_string(&plan.asset_index_path).map_err(|e| CoreError::io(plan.asset_index_path.display().to_string(), e))?;
    let asset_index = AssetIndex::parse(&asset_index_text)?;
    if asset_index.layout() == AssetLayout::VirtualLegacy {
        let _ = events.send(InstallEvent::Phase("Раскладка ассетов (legacy)"));
        let target = plan.assets_dir.join("virtual").join("legacy");
        std::fs::create_dir_all(&target).map_err(|e| CoreError::io(target.display().to_string(), e))?;
        for (source, dest) in asset_index.legacy_copy_plan(&plan.assets_objects_dir, &target) {
            if dest.exists() {
                continue;
            }
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).map_err(|e| CoreError::io(parent.display().to_string(), e))?;
            }
            if std::fs::hard_link(&source, &dest).is_err() {
                std::fs::copy(&source, &dest).map_err(|e| CoreError::io(dest.display().to_string(), e))?;
            }
        }
    }

    if let Some(extra) = &plan.forge {
        let _ = events.send(InstallEvent::Phase("Патчим клиент (Forge/NeoForge)"));
        let installer_bytes = std::fs::read(&extra.installer_path).map_err(|e| CoreError::io(extra.installer_path.display().to_string(), e))?;
        let java_executable = java::java_executable_path(&plan.java_home);
        let ctx = ForgeInstallContext {
            installer_bytes: &installer_bytes,
            profile: &extra.install_profile,
            root: &plan.version_dir,
            minecraft_jar: &plan.client_jar,
            minecraft_version: &extra.install_profile.minecraft,
            installer_path: &extra.installer_path,
            libraries_dir: &plan.libraries_dir,
            java_executable: &java_executable,
        };
        forge::run_client_processors(ctx).await?;
    }

    Ok(())
}

/// Для самых старых версий (alpha/beta/1.0–1.5.2, индекс ассетов
/// `pre-1.6`, флаг `map_to_resources`) игра читает ресурсы не из общего
/// `shared/assets/`, а прямо из `<gameDir>/resources/` — поэтому, в
/// отличие от остального плана (переиспользуется между инстансами одной
/// версии), этот шаг обязательно выполняется отдельно на каждый
/// инстанс, с его собственным `game_dir`. Ничего не делает для версий,
/// которым это не требуется (проверяет раскладку по индексу ассетов).
pub fn ensure_map_to_resources(plan: &InstallPlan, game_dir: &Path) -> Result<()> {
    let asset_index_text = std::fs::read_to_string(&plan.asset_index_path).map_err(|e| CoreError::io(plan.asset_index_path.display().to_string(), e))?;
    let asset_index = AssetIndex::parse(&asset_index_text)?;
    if asset_index.layout() != AssetLayout::MapToResources {
        return Ok(());
    }

    let target = game_dir.join("resources");
    std::fs::create_dir_all(&target).map_err(|e| CoreError::io(target.display().to_string(), e))?;
    for (source, dest) in asset_index.legacy_copy_plan(&plan.assets_objects_dir, &target) {
        if dest.exists() {
            continue;
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| CoreError::io(parent.display().to_string(), e))?;
        }
        if std::fs::hard_link(&source, &dest).is_err() {
            std::fs::copy(&source, &dest).map_err(|e| CoreError::io(dest.display().to_string(), e))?;
        }
    }
    Ok(())
}

/// Всё, что зависит от аккаунта/инстанса/настроек, а не от версии игры —
/// то, что `plan_install`/`execute_install` не знают и знать не должны.
pub struct LaunchParams {
    pub player_name: String,
    pub uuid: String,
    pub access_token: String,
    pub xuid: Option<String>,
    pub account_kind: AccountKind,
    pub game_directory: PathBuf,
    pub min_ram_mb: u32,
    pub max_ram_mb: u32,
    pub extra_jvm_args: Vec<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub quick_play: Option<QuickPlay>,
}

/// Собирает готовую к запуску команду из установленной версии (`plan`,
/// уже прошедший через `execute_install`) и параметров аккаунта/инстанса.
///
/// Клиентский jar не идёт в `-cp` для Forge/NeoForge на ModLauncher/
/// BootstrapLauncher (≥ примерно 1.13–1.17 в зависимости от версии) —
/// пропатченные классы игры находит сам FML по путям в `shared/libraries`
/// (см. `loaders::forge::run` — вывод процессоров кладётся туда же, куда
/// потом смотрит `${library_directory}`/`-p` из смерженных jvm-аргументов).
/// Определяем это по `mainClass`, а не только по типу загрузчика: старый
/// Forge (LaunchWrapper, до ModLauncher) собирает classpath как ваниль —
/// такие версии этим лаунчером пока не проверялись вживую.
fn uses_modlauncher_bootstrap(main_class: &str) -> bool {
    main_class.to_ascii_lowercase().contains("bootstraplauncher")
}

/// `--assetsDir`/`${game_assets}` должен указывать на РАЗНЫЕ каталоги в
/// зависимости от раскладки индекса ассетов — современные версии читают
/// объекты сами по хэшу из общего `shared/assets`, а вот версии до
/// 1.7.10 ожидают плоское дерево прямо по этому пути. Без этой развилки
/// самые старые версии (1.0 и старее, раскладка `map_to_resources`)
/// запускались бы с чёрным экраном и отсутствующими текстурами.
fn resolve_assets_root(plan: &InstallPlan, paths: &AppPaths, game_directory: &std::path::Path) -> PathBuf {
    let layout = std::fs::read_to_string(&plan.asset_index_path).ok().and_then(|text| AssetIndex::parse(&text).ok()).map(|idx| idx.layout()).unwrap_or(AssetLayout::Modern);
    match layout {
        AssetLayout::Modern => paths.assets_dir(),
        AssetLayout::VirtualLegacy => paths.assets_dir().join("virtual").join("legacy"),
        AssetLayout::MapToResources => game_directory.join("resources"),
    }
}

pub fn build_launch_command(plan: &InstallPlan, paths: &AppPaths, params: LaunchParams) -> LaunchCommand {
    let separator = if cfg!(windows) { ";" } else { ":" };
    let include_client_jar = match plan.loader {
        LoaderKind::Forge | LoaderKind::NeoForge => !uses_modlauncher_bootstrap(&plan.resolved.main_class),
        _ => true,
    };
    let classpath = launch::build_classpath(&plan.resolved, &paths.libraries_dir(), &plan.client_jar, include_client_jar, separator);
    let assets_root = resolve_assets_root(plan, paths, &params.game_directory);

    let ctx = LaunchContext {
        player_name: params.player_name,
        uuid: params.uuid,
        access_token: params.access_token,
        xuid: params.xuid,
        account_kind: params.account_kind,
        version_name: plan.resolved.id.clone(),
        version_type: "release".to_string(),
        game_directory: params.game_directory.clone(),
        assets_root,
        assets_index_name: plan.resolved.asset_index.id.clone(),
        natives_directory: plan.natives_dir.clone(),
        classpath,
        library_directory: paths.libraries_dir(),
        launcher_name: "DreamLauncher".to_string(),
        launcher_version: env!("CARGO_PKG_VERSION").to_string(),
        width: params.width,
        height: params.height,
        quick_play: params.quick_play,
        demo: false,
    };

    let jvm_args = launch::args::build_jvm_args(&plan.resolved, &ctx, params.min_ram_mb, params.max_ram_mb, &params.extra_jvm_args);
    let game_args = launch::args::build_game_args(&plan.resolved, &ctx);

    LaunchCommand {
        java_executable: java::java_executable_path(&plan.java_home),
        jvm_args,
        main_class: plan.resolved.main_class.clone(),
        game_args,
        working_directory: params.game_directory,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_resolved(name: &str) -> ResolvedVersion {
        let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(path).unwrap();
        let raw = serde_json::from_str(&text).unwrap();
        meta::merge_chain(vec![raw]).unwrap()
    }

    fn fake_plan(resolved: ResolvedVersion) -> InstallPlan {
        fake_plan_with_loader(resolved, LoaderKind::Vanilla)
    }

    fn fake_plan_with_loader(resolved: ResolvedVersion, loader: LoaderKind) -> InstallPlan {
        InstallPlan {
            resolved,
            loader,
            version_dir: PathBuf::from(r"C:\dream\shared\versions\1.21.1"),
            client_jar: PathBuf::from(r"C:\dream\shared\versions\1.21.1\1.21.1.jar"),
            natives_dir: PathBuf::from(r"C:\dream\shared\versions\1.21.1\natives-windows-x86_64"),
            java_home: PathBuf::from(r"C:\dream\shared\java\java-runtime-delta"),
            java_already_installed: true,
            download_tasks: Vec::new(),
            native_jars: Vec::new(),
            asset_index_path: PathBuf::from(r"C:\dream\shared\assets\indexes\17.json"),
            assets_objects_dir: PathBuf::from(r"C:\dream\shared\assets\objects"),
            assets_dir: PathBuf::from(r"C:\dream\shared\assets"),
            libraries_dir: PathBuf::from(r"C:\dream\shared\libraries"),
            forge: None,
        }
    }

    #[test]
    fn build_launch_command_wires_classpath_and_java() {
        let plan = fake_plan(fixture_resolved("version-1.21.1.json"));
        let paths = AppPaths::new(r"C:\dream");

        let cmd = build_launch_command(
            &plan,
            &paths,
            LaunchParams {
                player_name: "Steve".into(),
                uuid: "11111111-2222-3333-4444-555555555555".into(),
                access_token: "0".into(),
                xuid: None,
                account_kind: AccountKind::Offline,
                game_directory: PathBuf::from(r"C:\dream\instances\demo\.minecraft"),
                min_ram_mb: 1024,
                max_ram_mb: 4096,
                extra_jvm_args: vec![],
                width: None,
                height: None,
                quick_play: None,
            },
        );

        assert_eq!(cmd.java_executable, PathBuf::from(r"C:\dream\shared\java\java-runtime-delta\bin\java.exe"));
        assert_eq!(cmd.main_class, "net.minecraft.client.main.Main");
        assert!(cmd.jvm_args.iter().any(|a| a.contains("natives-windows-x86_64")));
        assert!(cmd.jvm_args.windows(2).any(|w| w[0] == "-cp" && w[1].ends_with("1.21.1.jar")));
        assert!(cmd.game_args.windows(2).any(|w| w[0] == "--username" && w[1] == "Steve"));
        assert_eq!(cmd.working_directory, PathBuf::from(r"C:\dream\instances\demo\.minecraft"));
    }

    #[test]
    fn version_dir_name_differs_by_loader() {
        let vanilla = VersionRef { mc_version: "1.21.1".into(), loader: LoaderKind::Vanilla, loader_version: None };
        assert_eq!(version_dir_name(&vanilla), "1.21.1");

        let fabric = VersionRef { mc_version: "1.21.1".into(), loader: LoaderKind::Fabric, loader_version: Some("0.16.9".into()) };
        assert_eq!(version_dir_name(&fabric), "1.21.1-fabric-0.16.9");
    }

    #[test]
    fn ensure_map_to_resources_copies_objects_into_game_dir_for_ancient_versions() {
        let dir = tempfile::tempdir().unwrap();
        let objects_dir = dir.path().join("objects");
        let game_dir = dir.path().join("instance").join(".minecraft");
        std::fs::create_dir_all(&game_dir).unwrap();

        // Кладём объекты по их реальным content-hash путям, как это
        // сделал бы `download_all` из настоящего pre-1.6 индекса — по
        // одному на каждую запись фикстуры, иначе copy/hard_link упадёт
        // на отсутствующем источнике (это и произошло при первой версии
        // теста: фикстура несёт 5 объектов, а фейковых файлов было 2).
        let all_hashes = [
            "0d000710b71ca9aafabd8f587768431d0b560b32", // READ_ME_I_AM_VERY_IMPORTANT
            "bdf48ef6b5d0d23bbb02e17d04865216179f510a", // icons/icon_16x16.png
            "92750c5f93c312ba9ab413d546f32190c56d6f1f", // icons/icon_32x32.png
            "991b421dfd401f115241601b2b373140a8d78572", // icons/minecraft.icns
            "50a59a4f56e4046701b758ddbb1c1587efa4cadf", // music/calm1.ogg
        ];
        for hash in all_hashes {
            let bucket = objects_dir.join(&hash[0..2]);
            std::fs::create_dir_all(&bucket).unwrap();
            std::fs::write(bucket.join(hash), b"fake-bytes").unwrap();
        }

        let asset_index_path = dir.path().join("indexes").join("pre-1.6.json");
        std::fs::create_dir_all(asset_index_path.parent().unwrap()).unwrap();
        let fixture = format!("{}/tests/fixtures/asset-index-map-to-resources.json", env!("CARGO_MANIFEST_DIR"));
        std::fs::copy(fixture, &asset_index_path).unwrap();

        let mut plan = fake_plan(fixture_resolved("version-1.21.1.json"));
        plan.asset_index_path = asset_index_path;
        plan.assets_objects_dir = objects_dir;

        ensure_map_to_resources(&plan, &game_dir).unwrap();

        assert_eq!(std::fs::read(game_dir.join("resources").join("READ_ME_I_AM_VERY_IMPORTANT")).unwrap(), b"fake-bytes");
        assert_eq!(std::fs::read(game_dir.join("resources").join("icons").join("icon_16x16.png")).unwrap(), b"fake-bytes");
        assert!(game_dir.join("resources").join("icons").join("icon_32x32.png").is_file());
        assert!(game_dir.join("resources").join("music").join("calm1.ogg").is_file());
    }

    #[test]
    fn ensure_map_to_resources_is_a_noop_for_modern_asset_layout() {
        // На современных версиях (asset index без map_to_resources) не
        // должно появляться никакой папки resources/ — это исключительно
        // поведение для версий ≤1.5.2.
        let dir = tempfile::tempdir().unwrap();
        let game_dir = dir.path().join("instance").join(".minecraft");
        std::fs::create_dir_all(&game_dir).unwrap();

        let asset_index_path = dir.path().join("indexes").join("17.json");
        std::fs::create_dir_all(asset_index_path.parent().unwrap()).unwrap();
        let fixture = format!("{}/tests/fixtures/asset-index-modern.json", env!("CARGO_MANIFEST_DIR"));
        std::fs::copy(fixture, &asset_index_path).unwrap();

        let mut plan = fake_plan(fixture_resolved("version-1.21.1.json"));
        plan.asset_index_path = asset_index_path;

        ensure_map_to_resources(&plan, &game_dir).unwrap();
        assert!(!game_dir.join("resources").exists());
    }

    #[tokio::test]
    async fn plain_resolve_version_rejects_forge_pointing_at_resolve_forge_like() {
        // `resolve_version` — только для vanilla/fabric/quilt; для Forge/
        // NeoForge нужен `resolve_forge_like` (свежий install_profile.json).
        let client = reqwest::Client::new();
        let v = VersionRef { mc_version: "1.20.1".into(), loader: LoaderKind::Forge, loader_version: Some("47.4.0".into()) };
        let err = resolve_version(&client, &v).await.unwrap_err();
        assert!(matches!(err, CoreError::Other(_)));
    }

    #[test]
    fn build_launch_command_excludes_client_jar_for_modlauncher_forge() {
        let mut resolved = fixture_resolved("version-1.21.1.json");
        resolved.main_class = "cpw.mods.bootstraplauncher.BootstrapLauncher".to_string();
        let plan = fake_plan_with_loader(resolved, LoaderKind::Forge);
        let paths = AppPaths::new(r"C:\dream");

        let cmd = build_launch_command(
            &plan,
            &paths,
            LaunchParams {
                player_name: "Steve".into(),
                uuid: "11111111-2222-3333-4444-555555555555".into(),
                access_token: "0".into(),
                xuid: None,
                account_kind: AccountKind::Offline,
                game_directory: PathBuf::from(r"C:\dream\instances\demo\.minecraft"),
                min_ram_mb: 1024,
                max_ram_mb: 4096,
                extra_jvm_args: vec![],
                width: None,
                height: None,
                quick_play: None,
            },
        );

        assert_eq!(cmd.main_class, "cpw.mods.bootstraplauncher.BootstrapLauncher");
        assert!(!cmd.jvm_args.windows(2).any(|w| w[0] == "-cp" && w[1].ends_with("1.21.1.jar")), "клиентский jar не должен идти в -cp для Forge на BootstrapLauncher");
    }

    /// Полный сквозной прогон против реального Mojang/Fabric API — не
    /// гоняется в обычном `cargo test` (нужна сеть и несколько сотен МБ
    /// на диск), запускается вручную:
    /// `cargo test -p dream-core install:: -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn installs_and_places_real_vanilla_version() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(dir.path());
        paths.ensure_shared_dirs().unwrap();

        let client = reqwest::Client::new();
        let v = VersionRef { mc_version: "1.21.1".into(), loader: LoaderKind::Vanilla, loader_version: None };

        let plan = plan_install(&client, &paths, &v).await.expect("plan_install must succeed");
        println!("total downloads: {}", plan.total_downloads());

        let (tx, mut rx) = mpsc::unbounded_channel();
        let drain = tokio::spawn(async move { while rx.recv().await.is_some() {} });
        execute_install(client, &plan, tx).await.expect("execute_install must succeed");
        drain.await.unwrap();

        assert!(plan.client_jar.is_file());
        assert!(java::is_installed(&plan.java_home));
        assert!(plan.natives_dir.is_dir());
    }

    /// Сквозная проверка "поставить и сыграть": использует настоящий
    /// каталог данных приложения (`%APPDATA%\DreamLauncher`) — то же
    /// место, которым пользуется собранное приложение, так что запуск
    /// заодно прогревает кэш для реального первого клика "Играть".
    /// Реально спавнит `java`/Minecraft и читает первые строки его
    /// stdout/stderr, подтверждая, что клиент действительно стартовал.
    /// `cargo test -p dream-core install:: -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn end_to_end_install_and_launch_using_real_app_data_dir() {
        let root = AppPaths::default_root().expect("не удалось определить %APPDATA%");
        let paths = AppPaths::new(root);
        paths.ensure_shared_dirs().unwrap();

        let client = reqwest::Client::new();
        let v = VersionRef { mc_version: "1.21.1".into(), loader: LoaderKind::Vanilla, loader_version: None };

        let plan = plan_install(&client, &paths, &v).await.expect("plan_install must succeed");
        println!("total downloads (skips already-valid files automatically): {}", plan.total_downloads());

        let (tx, mut rx) = mpsc::unbounded_channel();
        let drain = tokio::spawn(async move { while rx.recv().await.is_some() {} });
        execute_install(client, &plan, tx).await.expect("execute_install must succeed");
        drain.await.unwrap();

        let game_dir = paths.instances_dir().join("_verify").join(".minecraft");
        std::fs::create_dir_all(&game_dir).unwrap();

        let cmd = build_launch_command(
            &plan,
            &paths,
            LaunchParams {
                player_name: "Steve".into(),
                uuid: crate::auth::offline_uuid("Steve"),
                access_token: "0".into(),
                xuid: None,
                account_kind: AccountKind::Offline,
                game_directory: game_dir,
                min_ram_mb: 512,
                max_ram_mb: 2048,
                extra_jvm_args: vec![],
                width: None,
                height: None,
                quick_play: None,
            },
        );
        println!("java: {}", cmd.java_executable.display());
        println!("mainClass: {}", cmd.main_class);

        let mut rx = launch::spawn(&cmd).await.expect("java must spawn");
        let mut lines = Vec::new();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(45);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                println!("(таймаут ожидания — процесс, видимо, успешно висит в меню, это ок)");
                break;
            }
            match tokio::time::timeout(remaining, rx.recv()).await {
                Ok(Some(event)) => {
                    match &event {
                        launch::LaunchEvent::Stdout(l) | launch::LaunchEvent::Stderr(l) => println!("[game] {l}"),
                        launch::LaunchEvent::Exited { code } => println!("[game] exited with {code:?}"),
                    }
                    let is_exit = matches!(event, launch::LaunchEvent::Exited { .. });
                    lines.push(event);
                    if is_exit || lines.len() > 400 {
                        break;
                    }
                }
                _ => break,
            }
        }

        let saw_exit_immediately = matches!(lines.first(), Some(launch::LaunchEvent::Exited { .. }));
        assert!(!saw_exit_immediately, "процесс завершился сразу же, вместо запуска клиента — см. вывод выше");
        assert!(lines.len() > 3, "ожидали хоть какой-то вывод от Java/Minecraft, получили {} событий", lines.len());
    }

    /// То же самое, но для Forge — проверяет весь конвейер патчинга
    /// (install_profile.json → процессоры → пропатченный client.jar на
    /// своём maven-пути) на реальном installer-е Forge 1.20.1-47.4.0, а
    /// не только резолвинг метаданных. `--ignored --nocapture`.
    #[tokio::test]
    #[ignore]
    async fn end_to_end_install_and_launch_real_forge() {
        let root = AppPaths::default_root().expect("не удалось определить %APPDATA%");
        let paths = AppPaths::new(root);
        paths.ensure_shared_dirs().unwrap();

        let client = reqwest::Client::new();
        let v = VersionRef { mc_version: "1.20.1".into(), loader: LoaderKind::Forge, loader_version: Some("47.4.0".into()) };

        let plan = plan_install(&client, &paths, &v).await.expect("plan_install must succeed for forge");
        println!("total downloads: {}", plan.total_downloads());
        assert!(plan.forge.is_some(), "план должен нести install_profile для Forge");

        let (tx, mut rx) = mpsc::unbounded_channel();
        let drain = tokio::spawn(async move {
            while let Some(ev) = rx.recv().await {
                if let InstallEvent::Phase(p) = ev {
                    println!("[phase] {p}");
                }
            }
        });
        execute_install(client, &plan, tx).await.expect("execute_install (включая процессоры Forge) must succeed");
        drain.await.unwrap();

        // Патченный клиентский jar должен появиться ровно там, где его
        // ищет FML — по тем же maven-координатам, что и {PATCHED}.
        let patched = plan.libraries_dir.join("net/minecraftforge/forge/1.20.1-47.4.0/forge-1.20.1-47.4.0-client.jar");
        assert!(patched.is_file(), "ожидали пропатченный клиент по адресу {}", patched.display());
        println!("mainClass: {}", plan.resolved.main_class);
        assert!(uses_modlauncher_bootstrap(&plan.resolved.main_class));

        let game_dir = paths.instances_dir().join("_verify_forge").join(".minecraft");
        std::fs::create_dir_all(&game_dir).unwrap();

        let cmd = build_launch_command(
            &plan,
            &paths,
            LaunchParams {
                player_name: "Steve".into(),
                uuid: crate::auth::offline_uuid("Steve"),
                access_token: "0".into(),
                xuid: None,
                account_kind: AccountKind::Offline,
                game_directory: game_dir,
                min_ram_mb: 1024,
                max_ram_mb: 3072,
                extra_jvm_args: vec![],
                width: None,
                height: None,
                quick_play: None,
            },
        );
        println!("java: {}", cmd.java_executable.display());

        let mut rx = launch::spawn(&cmd).await.expect("java must spawn");
        let mut lines = Vec::new();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(90);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                println!("(таймаут — процесс, видимо, успешно висит в меню/загрузке модов, это ок)");
                break;
            }
            match tokio::time::timeout(remaining, rx.recv()).await {
                Ok(Some(event)) => {
                    match &event {
                        launch::LaunchEvent::Stdout(l) | launch::LaunchEvent::Stderr(l) => println!("[game] {l}"),
                        launch::LaunchEvent::Exited { code } => println!("[game] exited with {code:?}"),
                    }
                    let is_exit = matches!(event, launch::LaunchEvent::Exited { .. });
                    lines.push(event);
                    if is_exit || lines.len() > 800 {
                        break;
                    }
                }
                _ => break,
            }
        }

        let saw_exit_immediately = lines.len() < 5;
        assert!(!saw_exit_immediately, "слишком мало вывода — похоже, Forge упал сразу после старта, см. лог выше");
    }

    /// Та же проверка, что и для Forge, но на NeoForge — другой groupId в
    /// maven-координатах (`net.neoforged` вместо `net.minecraftforge`) и
    /// отдельный сервер метаданных, но тот же формат install_profile.json
    /// и тот же движок процессоров, без специального кода под NeoForge.
    /// `cargo test -p dream-core install:: -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn end_to_end_install_and_launch_real_neoforge() {
        let root = AppPaths::default_root().expect("не удалось определить %APPDATA%");
        let paths = AppPaths::new(root);
        paths.ensure_shared_dirs().unwrap();

        let client = reqwest::Client::new();
        let v = VersionRef { mc_version: "1.21.1".into(), loader: LoaderKind::NeoForge, loader_version: Some("21.1.99".into()) };

        let plan = plan_install(&client, &paths, &v).await.expect("plan_install must succeed for neoforge");
        println!("total downloads: {}", plan.total_downloads());
        assert!(plan.forge.is_some(), "план должен нести install_profile для NeoForge");

        let (tx, mut rx) = mpsc::unbounded_channel();
        let drain = tokio::spawn(async move {
            while let Some(ev) = rx.recv().await {
                if let InstallEvent::Phase(p) = ev {
                    println!("[phase] {p}");
                }
            }
        });
        execute_install(client, &plan, tx).await.expect("execute_install (включая процессоры NeoForge) must succeed");
        drain.await.unwrap();

        let patched = plan.libraries_dir.join("net/neoforged/neoforge/21.1.99/neoforge-21.1.99-client.jar");
        assert!(patched.is_file(), "ожидали пропатченный клиент по адресу {}", patched.display());
        println!("mainClass: {}", plan.resolved.main_class);
        assert!(uses_modlauncher_bootstrap(&plan.resolved.main_class));

        let game_dir = paths.instances_dir().join("_verify_neoforge").join(".minecraft");
        std::fs::create_dir_all(&game_dir).unwrap();

        let cmd = build_launch_command(
            &plan,
            &paths,
            LaunchParams {
                player_name: "Steve".into(),
                uuid: crate::auth::offline_uuid("Steve"),
                access_token: "0".into(),
                xuid: None,
                account_kind: AccountKind::Offline,
                game_directory: game_dir,
                min_ram_mb: 1024,
                max_ram_mb: 3072,
                extra_jvm_args: vec![],
                width: None,
                height: None,
                quick_play: None,
            },
        );
        println!("java: {}", cmd.java_executable.display());

        let mut rx = launch::spawn(&cmd).await.expect("java must spawn");
        let mut lines = Vec::new();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(90);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                println!("(таймаут — процесс, видимо, успешно висит в меню/загрузке модов, это ок)");
                break;
            }
            match tokio::time::timeout(remaining, rx.recv()).await {
                Ok(Some(event)) => {
                    match &event {
                        launch::LaunchEvent::Stdout(l) | launch::LaunchEvent::Stderr(l) => println!("[game] {l}"),
                        launch::LaunchEvent::Exited { code } => println!("[game] exited with {code:?}"),
                    }
                    let is_exit = matches!(event, launch::LaunchEvent::Exited { .. });
                    lines.push(event);
                    if is_exit || lines.len() > 800 {
                        break;
                    }
                }
                _ => break,
            }
        }

        let saw_exit_immediately = lines.len() < 5;
        assert!(!saw_exit_immediately, "слишком мало вывода — похоже, NeoForge упал сразу после старта, см. лог выше");
    }

    /// Самая старая играбельная раскладка ассетов: `map_to_resources`
    /// (индекс `pre-1.6`, всё ≤1.5.2 и часть более старых alpha/beta).
    /// Проверяет ровно то, из-за чего этот путь раньше был явно
    /// отключён: раскладку ассетов в `<gameDir>/resources/` и токен
    /// `${auth_session}`, которого нет у современных версий.
    /// `cargo test -p dream-core install:: -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn end_to_end_install_and_launch_minecraft_1_0() {
        let root = AppPaths::default_root().expect("не удалось определить %APPDATA%");
        let paths = AppPaths::new(root);
        paths.ensure_shared_dirs().unwrap();

        let client = reqwest::Client::new();
        let v = VersionRef { mc_version: "1.0".into(), loader: LoaderKind::Vanilla, loader_version: None };

        let plan = plan_install(&client, &paths, &v).await.expect("plan_install must succeed for 1.0");
        println!("total downloads: {}", plan.total_downloads());
        println!("mainClass: {}", plan.resolved.main_class);
        println!("minecraftArguments: {:?}", plan.resolved.minecraft_arguments);

        let (tx, mut rx) = mpsc::unbounded_channel();
        let drain = tokio::spawn(async move { while rx.recv().await.is_some() {} });
        execute_install(client, &plan, tx).await.expect("execute_install must succeed for 1.0");
        drain.await.unwrap();

        let game_dir = paths.instances_dir().join("_verify_1_0").join(".minecraft");
        std::fs::create_dir_all(&game_dir).unwrap();
        ensure_map_to_resources(&plan, &game_dir).expect("ensure_map_to_resources must succeed");

        // Настоящая раскладка ресурсов должна появиться именно в этом
        // инстансе, а не в общем shared/assets.
        let resources = game_dir.join("resources");
        assert!(resources.is_dir(), "ожидали resources/ в {}", game_dir.display());
        let has_any_file = std::fs::read_dir(&resources).unwrap().next().is_some();
        assert!(has_any_file, "resources/ пустая — раскладка ассетов не сработала");

        let cmd = build_launch_command(
            &plan,
            &paths,
            LaunchParams {
                player_name: "Steve".into(),
                uuid: crate::auth::offline_uuid("Steve"),
                access_token: "0".into(),
                xuid: None,
                account_kind: AccountKind::Offline,
                game_directory: game_dir.clone(),
                min_ram_mb: 512,
                max_ram_mb: 2048,
                extra_jvm_args: vec![],
                width: None,
                height: None,
                quick_play: None,
            },
        );
        println!("java: {}", cmd.java_executable.display());
        println!("game_args: {:?}", cmd.game_args);
        // `${auth_session}`/`${game_assets}` должны были подставиться —
        // если substitute не знал токен, он остался бы литералом "${...}".
        assert!(!cmd.game_args.iter().any(|a| a.contains("${")), "остался неразрешённый токен: {:?}", cmd.game_args);
        assert!(cmd.game_args.contains(&resources.to_string_lossy().into_owned()) || cmd.game_args.iter().any(|a| a.contains("resources")), "--assetsDir должен указывать на per-инстанс resources/");

        let mut rx = launch::spawn(&cmd).await.expect("java must spawn");
        let mut lines = Vec::new();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(45);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                println!("(таймаут — процесс, видимо, успешно висит в меню, это ок)");
                break;
            }
            match tokio::time::timeout(remaining, rx.recv()).await {
                Ok(Some(event)) => {
                    match &event {
                        launch::LaunchEvent::Stdout(l) | launch::LaunchEvent::Stderr(l) => println!("[game] {l}"),
                        launch::LaunchEvent::Exited { code } => println!("[game] exited with {code:?}"),
                    }
                    let is_exit = matches!(event, launch::LaunchEvent::Exited { .. });
                    lines.push(event);
                    if is_exit || lines.len() > 400 {
                        break;
                    }
                }
                _ => break,
            }
        }

        let saw_exit_immediately = matches!(lines.first(), Some(launch::LaunchEvent::Exited { .. }));
        assert!(!saw_exit_immediately, "процесс завершился сразу же вместо запуска клиента 1.0 — см. вывод выше");
        assert!(lines.len() > 3, "ожидали хоть какой-то вывод от Java/Minecraft 1.0, получили {} событий", lines.len());
    }
}
