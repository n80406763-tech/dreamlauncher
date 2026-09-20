use super::parse_loader;
use crate::dto::InstallProgressEvent;
use crate::error::{DreamError, Result};
use crate::state::AppState;
use dream_core::download::DownloadEvent;
use dream_core::install::{self, VersionRef};
use tauri::ipc::Channel;
use tokio::sync::mpsc;

/// Скачивает и раскладывает на диск всё, что нужно для запуска инстанса:
/// client.jar, библиотеки, натив-либы, ассеты и (при необходимости) Java.
/// Идемпотентна — уже валидные файлы просто пропускаются, поэтому её
/// безопасно вызывать перед каждым запуском игры.
#[tauri::command]
#[specta::specta]
pub async fn install_version(state: tauri::State<'_, AppState>, instance_id: String, channel: Channel<InstallProgressEvent>) -> Result<()> {
    let (slug, mc_version, loader, loader_version) = {
        let db = state.db.lock().expect("db mutex poisoned");
        db.query_row("SELECT slug, mc_version, loader, loader_version FROM instances WHERE id = ?1", [&instance_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, Option<String>>(3)?))
        })
        .map_err(|_| DreamError::Other(format!("инстанс {instance_id} не найден")))?
    };

    let version_ref = VersionRef { mc_version, loader: parse_loader(&loader)?, loader_version };

    let _ = channel.send(InstallProgressEvent::Phase { message: "Получаем метаданные версии…".into() });
    let plan = match install::plan_install(&state.http, &state.paths, &version_ref).await {
        Ok(p) => p,
        Err(e) => {
            let _ = channel.send(InstallProgressEvent::Error { message: e.to_string() });
            return Err(e.into());
        }
    };

    let total = plan.total_downloads() as u32;
    let _ = channel.send(InstallProgressEvent::Progress { done: 0, total });

    let (tx, mut rx) = mpsc::unbounded_channel();
    let forward_channel = channel.clone();
    let forward = tokio::spawn(async move {
        let mut done: u32 = 0;
        while let Some(event) = rx.recv().await {
            match event {
                install::InstallEvent::Phase(message) => {
                    let _ = forward_channel.send(InstallProgressEvent::Phase { message: message.to_string() });
                }
                install::InstallEvent::Download(DownloadEvent::ItemFailed { url, error }) => {
                    done += 1;
                    let _ = forward_channel.send(InstallProgressEvent::ItemFailed { url, error });
                    let _ = forward_channel.send(InstallProgressEvent::Progress { done, total });
                }
                install::InstallEvent::Download(_) => {
                    done += 1;
                    let _ = forward_channel.send(InstallProgressEvent::Progress { done, total });
                }
            }
        }
    });

    let result = install::execute_install(state.http.clone(), &plan, tx).await;
    let _ = forward.await;

    if let Err(e) = result {
        let _ = channel.send(InstallProgressEvent::Error { message: e.to_string() });
        return Err(e.into());
    }

    // Только для самых старых версий (map_to_resources, ≤1.5.2) — раскладка
    // ассетов прямо в папку ЭТОГО инстанса; для всех остальных версий
    // ничего не делает (см. doc-комментарий `ensure_map_to_resources`).
    if let Err(e) = install::ensure_map_to_resources(&plan, &state.paths.instance_game_dir(&slug)) {
        let _ = channel.send(InstallProgressEvent::Error { message: e.to_string() });
        return Err(e.into());
    }

    let _ = channel.send(InstallProgressEvent::Done);
    Ok(())
}
