use super::parse_loader;
use crate::dto::{InstalledShaderDto, ShaderSearchResultDto};
use crate::error::{DreamError, Result};
use crate::state::AppState;
use dream_core::download::{download_all, DownloadTask, DownloaderConfig};
use dream_core::loaders::LoaderKind;
use dream_core::mods::modrinth;
use dream_core::shaders;
use dream_core::storage::AppPaths;
use tokio::sync::mpsc;

fn instance_row(state: &AppState, instance_id: &str) -> Result<(String, String, String)> {
    let db = state.db.lock().expect("db mutex poisoned");
    db.query_row("SELECT slug, mc_version, loader FROM instances WHERE id = ?1", [instance_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))
        .map_err(|_| DreamError::Other(format!("инстанс {instance_id} не найден")))
}

/// Поиск шейдер-паков для версии игры инстанса.
#[tauri::command]
#[specta::specta]
pub async fn shaders_search(state: tauri::State<'_, AppState>, instance_id: String, query: String, offset: u32) -> Result<ShaderSearchResultDto> {
    let (_, mc_version, _) = instance_row(&state, &instance_id)?;
    let result = modrinth::search_shaders(&state.http, &query, &mc_version, 20, offset).await?;
    Ok(result.into())
}

#[tauri::command]
#[specta::specta]
pub fn shaders_list(state: tauri::State<'_, AppState>, instance_id: String) -> Result<Vec<InstalledShaderDto>> {
    let db = state.db.lock().expect("db mutex poisoned");

    // Получаем active_shader_id из instances
    let active_shader_id: Option<String> = db
        .query_row("SELECT active_shader_id FROM instances WHERE id = ?1", [&instance_id], |r| r.get(0))
        .ok();

    let mut stmt = db.prepare("SELECT project_id, version_id, filename FROM installed_content WHERE instance_id = ?1 AND content_type = 'shader' ORDER BY filename")?;
    let rows = stmt.query_map([&instance_id], |r| {
        let project_id: String = r.get(0)?;
        let is_active = active_shader_id.as_ref() == Some(&project_id);
        Ok(InstalledShaderDto {
            project_id,
            version_id: r.get(1)?,
            filename: r.get(2)?,
            is_active
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Устанавливает шейдер-пак. Автоматически проверяет наличие Iris/OptiFine.
#[tauri::command]
#[specta::specta]
pub async fn shaders_install(state: tauri::State<'_, AppState>, instance_id: String, project_id: String) -> Result<String> {
    let (slug, mc_version, loader) = instance_row(&state, &instance_id)?;
    let loader_kind = parse_loader(&loader)?;

    // Проверяем наличие загрузчика шейдеров
    let installed_mods: Vec<String> = {
        let db = state.db.lock().expect("db mutex poisoned");
        let mut stmt = db.prepare("SELECT project_id FROM installed_content WHERE instance_id = ?1 AND content_type = 'mod'")?;
        let rows = stmt.query_map([&instance_id], |r| r.get(0))?;
        rows.collect::<rusqlite::Result<_>>()?
    };

    if !shaders::has_shader_loader(&installed_mods) {
        if let Some((dep_id, dep_name)) = shaders::shader_loader_dependency(loader_kind) {
            return Err(DreamError::Other(format!(
                "Для работы шейдеров требуется {}. Установите его во вкладке 'Моды' (project_id: {})",
                dep_name, dep_id
            )));
        }
    }

    let shaderpacks_dir = state.paths.instance_game_dir(&slug).join("shaderpacks");
    std::fs::create_dir_all(&shaderpacks_dir)?;

    // Для шейдеров загрузчик не важен - работают на всех
    let versions = modrinth::list_versions(&state.http, &project_id, &[], &mc_version).await?;
    let best = modrinth::pick_best_version(&versions).ok_or_else(|| DreamError::Other(format!("для {project_id} нет версии, совместимой с {mc_version}")))?;
    let file = best.primary_file().ok_or_else(|| DreamError::Other(format!("у версии {} проекта {project_id} нет файлов", best.id)))?;

    let dest = shaderpacks_dir.join(&file.filename);
    let (tx, _rx) = mpsc::unbounded_channel();
    download_all(
        state.http.clone(),
        vec![DownloadTask { url: file.url.clone(), dest, sha1: Some(file.hashes.sha1.clone()), size: Some(file.size) }],
        DownloaderConfig::default(),
        tx,
    )
    .await?;

    {
        let conn = state.db.lock().expect("db mutex poisoned");
        conn.execute(
            "INSERT INTO installed_content (instance_id, project_id, version_id, filename, sha1, content_type)
             VALUES (?1, ?2, ?3, ?4, ?5, 'shader')
             ON CONFLICT(instance_id, project_id) DO UPDATE SET version_id = excluded.version_id, filename = excluded.filename, sha1 = excluded.sha1",
            rusqlite::params![instance_id, project_id, best.id, file.filename, file.hashes.sha1],
        )?;
    }

    Ok(project_id)
}

/// Устанавливает активный шейдер (None = отключить шейдеры).
#[tauri::command]
#[specta::specta]
pub fn shaders_set_active(state: tauri::State<'_, AppState>, instance_id: String, project_id: Option<String>) -> Result<()> {
    let db = state.db.lock().expect("db mutex poisoned");

    // Проверяем, что шейдер установлен (если указан)
    if let Some(ref pid) = project_id {
        let exists: bool = db
            .query_row(
                "SELECT 1 FROM installed_content WHERE instance_id = ?1 AND project_id = ?2 AND content_type = 'shader'",
                [&instance_id, pid],
                |_| Ok(true)
            )
            .unwrap_or(false);

        if !exists {
            return Err(DreamError::Other(format!("шейдер {pid} не установлен в этот инстанс")));
        }
    }

    db.execute(
        "UPDATE instances SET active_shader_id = ?1 WHERE id = ?2",
        rusqlite::params![project_id, instance_id]
    )?;

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn shaders_remove(state: tauri::State<'_, AppState>, instance_id: String, project_id: String) -> Result<()> {
    let (slug, _, _) = instance_row(&state, &instance_id)?;
    let shaderpacks_dir = state.paths.instance_game_dir(&slug).join("shaderpacks");

    let filename: String = {
        let db = state.db.lock().expect("db mutex poisoned");
        db.query_row(
            "SELECT filename FROM installed_content WHERE instance_id = ?1 AND project_id = ?2 AND content_type = 'shader'",
            [&instance_id, &project_id],
            |r| r.get(0)
        )
        .map_err(|_| DreamError::Other(format!("шейдер {project_id} не установлен в этот инстанс")))?
    };

    let file_path = shaderpacks_dir.join(&filename);
    if file_path.exists() {
        std::fs::remove_file(file_path)?;
    }

    {
        let db = state.db.lock().expect("db mutex poisoned");

        // Если это активный шейдер - деактивируем
        db.execute(
            "UPDATE instances SET active_shader_id = NULL WHERE id = ?1 AND active_shader_id = ?2",
            [&instance_id, &project_id]
        )?;

        db.execute(
            "DELETE FROM installed_content WHERE instance_id = ?1 AND project_id = ?2",
            [&instance_id, &project_id]
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn searches_real_shaders_from_modrinth() {
        let client = reqwest::Client::new();
        let result = modrinth::search_shaders(&client, "BSL", "1.21.1", 20, 0).await.expect("поиск должен пройти");
        assert!(result.total_hits > 0);
        assert!(result.hits.iter().any(|h| h.title.contains("BSL") || h.title.contains("Shaders")));
    }
}
