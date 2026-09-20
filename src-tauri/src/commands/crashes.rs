use crate::dto::CrashReportDto;
use crate::error::{DreamError, Result};
use crate::state::AppState;
use dream_core::launch::{find_recent_crashes, read_crash_report};

/// Ищет свежие крэш-репорты для инстанса (до 5 штук).
#[tauri::command]
#[specta::specta]
pub fn crashes_list(
    state: tauri::State<'_, AppState>,
    instance_id: String,
) -> Result<Vec<CrashReportDto>> {
    let db = state.db.lock().expect("db mutex poisoned");
    let slug: String = db
        .query_row(
            "SELECT slug FROM instances WHERE id = ?1",
            [&instance_id],
            |r| r.get(0),
        )
        .map_err(|_| DreamError::Other(format!("инстанс {instance_id} не найден")))?;
    drop(db);

    let game_dir = state.paths.instance_game_dir(&slug);
    let reports = find_recent_crashes(&game_dir, 5)?;

    let now = std::time::SystemTime::now();
    Ok(reports
        .into_iter()
        .map(|r| {
            let modified_secs = now.duration_since(r.modified).unwrap_or_default().as_secs();
            CrashReportDto {
                path: r.path.display().to_string(),
                kind: match r.kind {
                    dream_core::launch::CrashKind::JvmFatal => "jvm_fatal".to_string(),
                    dream_core::launch::CrashKind::GameCrash => "game_crash".to_string(),
                },
                preview: r.preview,
                modified: modified_secs.min(u32::MAX as u64) as u32,
            }
        })
        .collect())
}

/// Читает полное содержимое крэш-репорта по пути.
#[tauri::command]
#[specta::specta]
pub fn crash_read(path: String) -> Result<String> {
    let content = read_crash_report(std::path::Path::new(&path))?;
    Ok(content)
}
