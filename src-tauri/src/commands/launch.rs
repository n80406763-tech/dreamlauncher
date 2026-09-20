use super::parse_loader;
use crate::dto::LaunchEventDto;
use crate::error::{DreamError, Result};
use crate::state::AppState;
use dream_core::install::{self, LaunchParams, VersionRef};
use dream_core::launch::{self, AccountKind};
use tauri::ipc::Channel;

/// Запускает уже установленную (см. `install_version`) версию. Не
/// проверяет повторно наличие файлов на диске — если что-то отсутствует,
/// Java просто упадёт с понятной ошибкой в stderr, который уходит в канал.
#[tauri::command]
#[specta::specta]
pub async fn launch_instance(state: tauri::State<'_, AppState>, instance_id: String, channel: Channel<LaunchEventDto>) -> Result<()> {
    let (slug, mc_version, loader, loader_version, min_ram_mb, max_ram_mb, extra_jvm_args) = {
        let db = state.db.lock().expect("db mutex poisoned");
        db.query_row(
            "SELECT slug, mc_version, loader, loader_version, min_ram_mb, max_ram_mb, extra_jvm_args FROM instances WHERE id = ?1",
            [&instance_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, String>(6)?,
                ))
            },
        )
        .map_err(|_| DreamError::Other(format!("инстанс {instance_id} не найден")))?
    };

    let (account_id, username, account_kind) = {
        let db = state.db.lock().expect("db mutex poisoned");
        db.query_row("SELECT id, username, kind FROM accounts WHERE is_active = 1", [], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))
            .map_err(|_| DreamError::Other("нет активного аккаунта — добавьте его на экране «Аккаунты»".into()))?
    };
    if account_kind != "offline" {
        return Err(DreamError::Other("запуск с Microsoft-аккаунтом пока не поддерживается (нужна авторизация, см. план, M3) — переключитесь на офлайн-профиль".into()));
    }

    let version_ref = VersionRef { mc_version, loader: parse_loader(&loader)?, loader_version };

    let _ = channel.send(LaunchEventDto::Starting);
    let plan = install::plan_install(&state.http, &state.paths, &version_ref).await?;

    let cmd = install::build_launch_command(
        &plan,
        &state.paths,
        LaunchParams {
            player_name: username,
            // `account_id` — это уже сам детерминированный офлайн-UUID
            // (см. `account_add_offline`: id = offline_uuid(&name)), а не
            // случайный идентификатор — пересчитывать его не нужно.
            uuid: account_id,
            access_token: "0".into(),
            xuid: None,
            account_kind: AccountKind::Offline,
            game_directory: state.paths.instance_game_dir(&slug),
            min_ram_mb: min_ram_mb as u32,
            max_ram_mb: max_ram_mb as u32,
            extra_jvm_args: extra_jvm_args.split_whitespace().map(str::to_string).collect(),
            width: None,
            height: None,
            quick_play: None,
        },
    );

    let mut rx = launch::spawn(&cmd).await.map_err(|e| DreamError::Other(format!("не удалось запустить Java: {e}")))?;
    while let Some(event) = rx.recv().await {
        let dto = match event {
            launch::LaunchEvent::Stdout(line) => LaunchEventDto::Stdout { line },
            launch::LaunchEvent::Stderr(line) => LaunchEventDto::Stderr { line },
            launch::LaunchEvent::Exited { code } => LaunchEventDto::Exited { code },
        };
        let _ = channel.send(dto);
    }

    Ok(())
}
