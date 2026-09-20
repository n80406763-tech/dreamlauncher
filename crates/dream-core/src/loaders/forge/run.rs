//! Выполнение процессоров патчинга Forge/NeoForge: резолвит `data`,
//! запускает `processors` по очереди (`java -cp <classpath> <Main-Class>
//! <args>`), проверяет `outputs` по sha1. Результат — пропатченные
//! артефакты (client/slim/extra/srg) на своих maven-путях в
//! `shared/libraries/` — их находит рантайм-загрузчик FML по тем же
//! путям, поэтому после этого шага ничего больше собирать не нужно (см.
//! `install::build_launch_command`, `include_client_jar = false` для
//! Forge/NeoForge).

use super::data_resolver::{maven_path, parse_data_value, substitute, DataValue, ProcessorContext};
use super::install_profile::{InstallProfile, ProcessorEntry};
use super::installer::{read_main_class, read_zip_entry_bytes};
use crate::download::sha1_hex_file;
use crate::error::{CoreError, Result};
use crate::meta::version_json::MavenCoords;
use std::path::Path;

/// Аргумент процессора — это либо `{TOKEN}` (см. `substitute`), либо
/// ЦЕЛИКОМ maven-координаты в квадратных скобках вроде
/// `[de.oceanlabs.mcp:mcp_config:1.20.1-...@zip]` (в отличие от `data`,
/// где то же самое в круглых... то есть фигурных скобках оборачивает
/// `{TOKEN}` — здесь квадратные скобки идут прямо в списке `args`, не
/// через `data`), либо обычная строка. Оба варианта встречаются в одной
/// и той же реальной установке (Forge 1.20.1: `MCP_DATA`-процессор
/// получает `[de.oceanlabs.mcp:mcp_config:...@zip]` прямо в `args`).
fn resolve_processor_arg(raw: &str, ctx: &ProcessorContext, libraries_dir: &Path) -> std::result::Result<String, String> {
    if let DataValue::Maven(coords) = parse_data_value(raw) {
        return Ok(maven_path(&coords, libraries_dir).to_string_lossy().into_owned());
    }
    substitute(raw, ctx)
}

fn classpath_separator() -> &'static str {
    if cfg!(windows) {
        ";"
    } else {
        ":"
    }
}

/// Всё, что нужно, чтобы прогнать процессоры одной установки.
pub struct ForgeInstallContext<'a> {
    pub installer_bytes: &'a [u8],
    pub profile: &'a InstallProfile,
    /// Рабочий каталог версии (`shared/versions/<id>/`) — сюда
    /// извлекаются файлы `data/*` из installer-а на время установки.
    pub root: &'a Path,
    pub minecraft_jar: &'a Path,
    pub minecraft_version: &'a str,
    pub installer_path: &'a Path,
    pub libraries_dir: &'a Path,
    pub java_executable: &'a Path,
}

/// Резолвит `data`-секцию (сторона client) в конкретные строки: maven-
/// координаты → путь в `shared/libraries` (с созданием родительского
/// каталога — инструменты процессоров сами каталоги не создают), литералы
/// — как есть, файлы installer-а — извлекаются на диск во временный
/// каталог `<root>/_forge_data/`.
fn build_data_map(ctx: &ForgeInstallContext) -> Result<std::collections::HashMap<String, String>> {
    let mut data = std::collections::HashMap::new();
    let extract_dir = ctx.root.join("_forge_data");

    for (key, entry) in &ctx.profile.data {
        let value = parse_data_value(&entry.client);
        let resolved = match value {
            DataValue::Maven(coords) => {
                let path = maven_path(&coords, ctx.libraries_dir);
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| CoreError::io(parent.display().to_string(), e))?;
                }
                path.to_string_lossy().into_owned()
            }
            DataValue::Literal(s) => s,
            DataValue::InstallerPath(entry_name) => {
                let name = entry_name.trim_start_matches('/');
                let bytes = read_zip_entry_bytes(ctx.installer_bytes, name)?;
                std::fs::create_dir_all(&extract_dir).map_err(|e| CoreError::io(extract_dir.display().to_string(), e))?;
                let dest = extract_dir.join(name.replace('/', "_"));
                std::fs::write(&dest, &bytes).map_err(|e| CoreError::io(dest.display().to_string(), e))?;
                dest.to_string_lossy().into_owned()
            }
        };
        data.insert(key.clone(), resolved);
    }

    Ok(data)
}

async fn run_one_processor(processor: &ProcessorEntry, proc_ctx: &ProcessorContext, libraries_dir: &Path, java_executable: &Path) -> Result<()> {
    let jar_coords = MavenCoords::parse(&processor.jar).ok_or_else(|| CoreError::Other(format!("не удалось разобрать координаты процессора: {}", processor.jar)))?;
    let jar_path = maven_path(&jar_coords, libraries_dir);
    let jar_bytes = std::fs::read(&jar_path).map_err(|e| CoreError::io(jar_path.display().to_string(), e))?;
    let main_class = read_main_class(&jar_bytes)?;

    let mut classpath_entries = vec![jar_path.to_string_lossy().into_owned()];
    for entry in &processor.classpath {
        let coords = MavenCoords::parse(entry).ok_or_else(|| CoreError::Other(format!("не удалось разобрать координаты в classpath процессора: {entry}")))?;
        classpath_entries.push(maven_path(&coords, libraries_dir).to_string_lossy().into_owned());
    }
    let classpath = classpath_entries.join(classpath_separator());

    let mut args = Vec::with_capacity(processor.args.len());
    for raw in &processor.args {
        args.push(resolve_processor_arg(raw, proc_ctx, libraries_dir).map_err(CoreError::Other)?);
    }

    let output = tokio::process::Command::new(java_executable)
        .arg("-cp")
        .arg(&classpath)
        .arg(&main_class)
        .args(&args)
        .output()
        .await
        .map_err(|e| CoreError::io(java_executable.display().to_string(), e))?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CoreError::Other(format!(
            "процессор {} завершился с кодом {:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
            processor.jar,
            output.status.code()
        )));
    }

    for (path_token, sha_token) in &processor.outputs {
        let path_str = substitute(path_token, proc_ctx).map_err(CoreError::Other)?;
        let expected_sha = substitute(sha_token, proc_ctx).map_err(CoreError::Other)?;
        let actual = sha1_hex_file(Path::new(&path_str)).await.map_err(|e| CoreError::io(path_str.clone(), e))?;
        if actual != expected_sha {
            return Err(CoreError::ChecksumMismatch { path: path_str, expected: expected_sha, actual });
        }
    }

    Ok(())
}

/// Прогоняет все клиентские процессоры по очереди — именно по очереди,
/// не параллельно: поздние процессоры читают файлы, которые записали
/// более ранние (см. пример в module doc `install_profile.rs`).
pub async fn run_client_processors(ctx: ForgeInstallContext<'_>) -> Result<()> {
    let data = build_data_map(&ctx)?;
    let proc_ctx = ProcessorContext {
        root: ctx.root.to_path_buf(),
        installer: ctx.installer_path.to_path_buf(),
        minecraft_jar: ctx.minecraft_jar.to_path_buf(),
        minecraft_version: ctx.minecraft_version.to_string(),
        library_dir: ctx.libraries_dir.to_path_buf(),
        data,
    };

    for processor in ctx.profile.client_processors() {
        run_one_processor(processor, &proc_ctx, ctx.libraries_dir, ctx.java_executable).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loaders::forge::InstallProfile;

    fn fixture_profile() -> InstallProfile {
        let path = format!("{}/tests/fixtures/forge-1.20.1-install_profile.json", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(path).unwrap();
        InstallProfile::parse(&text).unwrap()
    }

    #[test]
    fn build_data_map_resolves_maven_and_literal_without_installer_files() {
        // Только data-ключи без InstallerPath (BINPATCH использует
        // installer_bytes, поэтому проверяем отдельно на реальном
        // installer-е в install::tests, `#[ignore]`) — здесь достаточно
        // подтвердить, что Maven/Literal резолвятся и создают каталоги.
        let profile = fixture_profile();
        let dir = tempfile::tempdir().unwrap();
        let libraries_dir = dir.path().join("libraries");
        let root = dir.path().join("version");
        std::fs::create_dir_all(&root).unwrap();

        // BINPATCH ссылается на installer — подменим installer_bytes на
        // архив с нужными записями, чтобы весь data-набор резолвился.
        let installer_zip = build_installer_stub();

        let ctx = ForgeInstallContext {
            installer_bytes: &installer_zip,
            profile: &profile,
            root: &root,
            minecraft_jar: Path::new("client.jar"),
            minecraft_version: "1.20.1",
            installer_path: Path::new("installer.jar"),
            libraries_dir: &libraries_dir,
            java_executable: Path::new("java"),
        };

        let data = build_data_map(&ctx).unwrap();

        // MAPPINGS -> maven coords -> путь под libraries_dir, каталог создан.
        let mappings_path = data.get("MAPPINGS").expect("MAPPINGS должен резолвиться");
        assert!(Path::new(mappings_path).starts_with(&libraries_dir));
        assert!(Path::new(mappings_path).parent().unwrap().is_dir());

        // MC_SLIM_SHA -> литерал без кавычек.
        let sha = data.get("MC_SLIM_SHA").unwrap();
        assert!(!sha.starts_with('\''));

        // BINPATCH -> файл, реально извлечённый из installer-а.
        let binpatch = data.get("BINPATCH").unwrap();
        assert!(Path::new(binpatch).is_file());
        assert_eq!(std::fs::read(binpatch).unwrap(), b"fake-binpatch-bytes");
    }

    fn build_installer_stub() -> Vec<u8> {
        use std::io::Write;
        use zip::write::SimpleFileOptions;
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut cursor);
            let options = SimpleFileOptions::default();
            writer.start_file("data/client.lzma", options).unwrap();
            writer.write_all(b"fake-binpatch-bytes").unwrap();
            writer.finish().unwrap();
        }
        cursor.into_inner()
    }
}
