use super::parse_loader;
use crate::dto::{InstalledModDto, ModSearchResultDto};
use crate::error::{DreamError, Result};
use crate::state::AppState;
use dream_core::download::{download_all, DownloadTask, DownloaderConfig};
use dream_core::mods::modrinth::{self, DependencyType};
use dream_core::storage::AppPaths;
use std::collections::HashSet;
use tokio::sync::mpsc;

fn instance_row(state: &AppState, instance_id: &str) -> Result<(String, String, String)> {
    let db = state.db.lock().expect("db mutex poisoned");
    db.query_row(
        "SELECT slug, mc_version, loader FROM instances WHERE id = ?1",
        [instance_id],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        },
    )
    .map_err(|_| DreamError::Other(format!("инстанс {instance_id} не найден")))
}

/// Поиск модов, уже отфильтрованный под версию игры и загрузчик инстанса.
#[tauri::command]
#[specta::specta]
pub async fn mods_search(
    state: tauri::State<'_, AppState>,
    instance_id: String,
    query: String,
    offset: u32,
) -> Result<ModSearchResultDto> {
    let (_, mc_version, loader) = instance_row(&state, &instance_id)?;
    let loader_kind = parse_loader(&loader)?;
    let result = modrinth::search(
        &state.http,
        &query,
        &mc_version,
        loader_kind.modrinth_loader_facets(),
        20,
        offset,
    )
    .await?;
    Ok(result.into())
}

#[tauri::command]
#[specta::specta]
pub fn mods_list(
    state: tauri::State<'_, AppState>,
    instance_id: String,
) -> Result<Vec<InstalledModDto>> {
    let db = state.db.lock().expect("db mutex poisoned");
    let mut stmt = db.prepare("SELECT project_id, version_id, filename, enabled FROM installed_content WHERE instance_id = ?1 AND content_type = 'mod' ORDER BY filename")?;
    let rows = stmt.query_map([&instance_id], |r| {
        Ok(InstalledModDto {
            project_id: r.get(0)?,
            version_id: r.get(1)?,
            filename: r.get(2)?,
            enabled: r.get::<_, i64>(3)? != 0,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Инстанс-специфичный контекст, нужный, чтобы поставить в него мод —
/// сгруппирован в структуру, а не передан по одному полю, чтобы не
/// заводить функцию с десятком параметров (`clippy::too_many_arguments`).
struct ModInstallInstance<'a> {
    slug: &'a str,
    mc_version: &'a str,
    loader: &'a str,
    facets: &'a [&'a str],
    instance_id: &'a str,
}

/// Ставит мод и рекурсивно все его обязательные зависимости (Fabric API
/// и т.п.) — без отдельного подтверждающего диалога (упрощение v1, см.
/// план: там предполагался список "будет доустановлено" перед подтверждением).
/// Вынесена из тонкой `#[tauri::command]`-обёртки, чтобы тестироваться
/// без реального `tauri::State` — только `reqwest::Client`/`Mutex<Connection>`/`AppPaths`.
/// Блокировка БД берётся только на сам `INSERT` внутри цикла, а не на
/// всё время функции — иначе она держалась бы поверх сетевых запросов
/// и замораживала остальные команды (`accounts_list`, `instances_list`, …).
async fn install_mod_with_deps(
    client: &reqwest::Client,
    db: &std::sync::Mutex<rusqlite::Connection>,
    paths: &AppPaths,
    instance: ModInstallInstance<'_>,
    root_project_id: String,
) -> Result<Vec<String>> {
    let ModInstallInstance {
        slug,
        mc_version,
        loader,
        facets,
        instance_id,
    } = instance;
    let mods_dir = paths.instance_game_dir(slug).join("mods");

    let mut installed = Vec::new();
    let mut visited = HashSet::new();
    let mut queue = vec![root_project_id];

    while let Some(current) = queue.pop() {
        if !visited.insert(current.clone()) {
            continue;
        }

        let versions = modrinth::list_versions(client, &current, facets, mc_version).await?;
        let best = modrinth::pick_best_version(&versions).ok_or_else(|| {
            DreamError::Other(format!(
                "для {current} нет версии, совместимой с {mc_version} ({loader})"
            ))
        })?;
        let file = best.primary_file().ok_or_else(|| {
            DreamError::Other(format!("у версии {} проекта {current} нет файлов", best.id))
        })?;

        let dest = mods_dir.join(&file.filename);
        let (tx, _rx) = mpsc::unbounded_channel();
        download_all(
            client.clone(),
            vec![DownloadTask {
                url: file.url.clone(),
                dest: dest.clone(),
                sha1: Some(file.hashes.sha1.clone()),
                size: Some(file.size),
            }],
            DownloaderConfig::default(),
            tx,
        )
        .await?;

        {
            let conn = db.lock().expect("db mutex poisoned");
            conn.execute(
                "INSERT INTO installed_content (instance_id, project_id, version_id, filename, sha1, enabled, content_type) VALUES (?1, ?2, ?3, ?4, ?5, 1, 'mod')
                 ON CONFLICT(instance_id, project_id) DO UPDATE SET version_id = excluded.version_id, filename = excluded.filename, sha1 = excluded.sha1, enabled = 1",
                rusqlite::params![instance_id, current, best.id, file.filename, file.hashes.sha1],
            )?;
        }

        installed.push(current.clone());

        for dep in &best.dependencies {
            if dep.dependency_type == DependencyType::Required {
                if let Some(dep_project) = &dep.project_id {
                    queue.push(dep_project.clone());
                }
            }
        }
    }

    Ok(installed)
}

#[tauri::command]
#[specta::specta]
pub async fn mods_install(
    state: tauri::State<'_, AppState>,
    instance_id: String,
    project_id: String,
) -> Result<Vec<String>> {
    let (slug, mc_version, loader) = instance_row(&state, &instance_id)?;
    let loader_kind = parse_loader(&loader)?;
    let facets = loader_kind.modrinth_loader_facets();
    let instance = ModInstallInstance {
        slug: &slug,
        mc_version: &mc_version,
        loader: &loader,
        facets,
        instance_id: &instance_id,
    };
    install_mod_with_deps(&state.http, &state.db, &state.paths, instance, project_id).await
}

fn mod_row(state: &AppState, instance_id: &str, project_id: &str) -> Result<(String, bool)> {
    let db = state.db.lock().expect("db mutex poisoned");
    db.query_row("SELECT filename, enabled FROM installed_content WHERE instance_id = ?1 AND project_id = ?2 AND content_type = 'mod'", [instance_id, project_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? != 0)))
        .map_err(|_| DreamError::Other(format!("мод {project_id} не установлен в этот инстанс")))
}

/// Выключенные моды переименовываются в `<файл>.disabled` — Minecraft
/// такие просто не подхватывает, без удаления с диска.
#[tauri::command]
#[specta::specta]
pub fn mods_toggle(
    state: tauri::State<'_, AppState>,
    instance_id: String,
    project_id: String,
) -> Result<()> {
    let (slug, _, _) = instance_row(&state, &instance_id)?;
    let (filename, enabled) = mod_row(&state, &instance_id, &project_id)?;
    let mods_dir = state.paths.instance_game_dir(&slug).join("mods");

    let (from, to) = if enabled {
        (
            mods_dir.join(&filename),
            mods_dir.join(format!("{filename}.disabled")),
        )
    } else {
        (
            mods_dir.join(format!("{filename}.disabled")),
            mods_dir.join(&filename),
        )
    };
    if from.exists() {
        std::fs::rename(&from, &to)?;
    }

    let db = state.db.lock().expect("db mutex poisoned");
    db.execute(
        "UPDATE installed_content SET enabled = ?1 WHERE instance_id = ?2 AND project_id = ?3",
        rusqlite::params![!enabled as i64, instance_id, project_id],
    )?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn mods_remove(
    state: tauri::State<'_, AppState>,
    instance_id: String,
    project_id: String,
) -> Result<()> {
    let (slug, _, _) = instance_row(&state, &instance_id)?;
    let (filename, _) = mod_row(&state, &instance_id, &project_id)?;
    let mods_dir = state.paths.instance_game_dir(&slug).join("mods");

    for candidate in [
        mods_dir.join(&filename),
        mods_dir.join(format!("{filename}.disabled")),
    ] {
        if candidate.exists() {
            std::fs::remove_file(candidate)?;
        }
    }

    let db = state.db.lock().expect("db mutex poisoned");
    db.execute(
        "DELETE FROM installed_content WHERE instance_id = ?1 AND project_id = ?2",
        [&instance_id, &project_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Реальный прогон против живого Modrinth API: ставит Sodium (Fabric,
    /// 1.21.1) во временный инстанс и проверяет, что файл лёг на диск с
    /// правильным содержимым, а строка в БД появилась и корректно
    /// обновляется повторной установкой (`ON CONFLICT`).
    /// `cargo test -p dreamlauncher commands::mods:: -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn installs_real_mod_from_modrinth_and_upserts_db_row() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(dir.path());
        paths.ensure_shared_dirs().unwrap();
        let db = std::sync::Mutex::new(dream_core::storage::open(&paths.db_path()).unwrap());
        {
            let conn = db.lock().unwrap();
            conn.execute(
                "INSERT INTO instances (id, slug, name, mc_version, loader) VALUES ('inst-1', 'demo', 'Demo', '1.21.1', 'fabric')",
                [],
            )
            .unwrap();
        }
        std::fs::create_dir_all(paths.instance_game_dir("demo")).unwrap();

        let client = reqwest::Client::new();
        let instance = ModInstallInstance {
            slug: "demo",
            mc_version: "1.21.1",
            loader: "fabric",
            facets: &["fabric"],
            instance_id: "inst-1",
        };
        // Sodium — реальный project_id с Modrinth (см. crates/dream-core/tests/fixtures/modrinth-search-sodium.json).
        let installed =
            install_mod_with_deps(&client, &db, &paths, instance, "AANobbMI".to_string())
                .await
                .expect("установка должна пройти");

        assert!(installed.contains(&"AANobbMI".to_string()));

        let mods_dir = paths.instance_game_dir("demo").join("mods");
        let files: Vec<_> = std::fs::read_dir(&mods_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(files.len(), 1, "ожидали ровно один установленный jar");
        assert!(files[0].file_name().to_string_lossy().contains("sodium"));

        {
            // Блок, а не отдельный `drop()` — так guard гарантированно не
            // переживает следующий `.await` ниже (clippy::await_holding_lock).
            let conn = db.lock().unwrap();
            let (version_id, filename, enabled): (String, String, i64) = conn
                .query_row("SELECT version_id, filename, enabled FROM installed_content WHERE instance_id = 'inst-1' AND project_id = 'AANobbMI'", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .unwrap();
            assert!(!version_id.is_empty());
            assert!(filename.contains("sodium"));
            assert_eq!(enabled, 1);
        }

        // Повторная установка — тот же файл, ON CONFLICT не должен упасть.
        let instance = ModInstallInstance {
            slug: "demo",
            mc_version: "1.21.1",
            loader: "fabric",
            facets: &["fabric"],
            instance_id: "inst-1",
        };
        install_mod_with_deps(&client, &db, &paths, instance, "AANobbMI".to_string())
            .await
            .expect("повторная установка должна пройти");
    }
}
