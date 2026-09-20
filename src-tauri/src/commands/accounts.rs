use crate::dto::{AccountDto, AuthProgressDto};
use crate::error::{DreamError, Result};
use crate::state::AppState;
use dream_core::auth::{
    authenticate_microsoft, check_minecraft_entitlements, is_valid_player_name, offline_uuid,
    AuthProgress, TokenVault,
};
use tauri::ipc::Channel;

fn row_to_dto(row: &rusqlite::Row) -> rusqlite::Result<AccountDto> {
    Ok(AccountDto {
        id: row.get(0)?,
        username: row.get(1)?,
        kind: row.get(2)?,
        is_active: row.get::<_, i64>(3)? != 0,
    })
}

#[tauri::command]
#[specta::specta]
pub fn accounts_list(state: tauri::State<'_, AppState>) -> Result<Vec<AccountDto>> {
    let db = state.db.lock().expect("db mutex poisoned");
    let mut stmt =
        db.prepare("SELECT id, username, kind, is_active FROM accounts ORDER BY created_at")?;
    let rows = stmt.query_map([], row_to_dto)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Добавляет офлайн-профиль: имя + детерминированный UUID, без сети.
/// Первый добавленный аккаунт автоматически становится активным.
#[tauri::command]
#[specta::specta]
pub fn account_add_offline(state: tauri::State<'_, AppState>, name: String) -> Result<AccountDto> {
    if !is_valid_player_name(&name) {
        return Err(DreamError::Other(format!(
            "недопустимое имя игрока: {name} (3-16 символов, латиница/цифры/подчёркивание)"
        )));
    }
    let id = offline_uuid(&name);

    let db = state.db.lock().expect("db mutex poisoned");
    let existing_active: i64 = db.query_row(
        "SELECT COUNT(*) FROM accounts WHERE is_active = 1",
        [],
        |r| r.get(0),
    )?;
    let is_active = existing_active == 0;

    db.execute(
        "INSERT INTO accounts (id, username, kind, is_active) VALUES (?1, ?2, 'offline', ?3)
         ON CONFLICT(id) DO UPDATE SET username = excluded.username",
        rusqlite::params![id, name, is_active as i64],
    )?;

    Ok(AccountDto {
        id,
        username: name,
        kind: "offline".into(),
        is_active,
    })
}

#[tauri::command]
#[specta::specta]
pub fn account_set_active(state: tauri::State<'_, AppState>, id: String) -> Result<()> {
    let db = state.db.lock().expect("db mutex poisoned");
    db.execute("UPDATE accounts SET is_active = 0", [])?;
    let changed = db.execute("UPDATE accounts SET is_active = 1 WHERE id = ?1", [&id])?;
    if changed == 0 {
        return Err(DreamError::Other(format!("аккаунт {id} не найден")));
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn account_remove(state: tauri::State<'_, AppState>, id: String) -> Result<()> {
    let db = state.db.lock().expect("db mutex poisoned");
    db.execute("DELETE FROM accounts WHERE id = ?1", [&id])?;
    drop(db);
    // Идемпотентно и для офлайн-аккаунтов, у которых секрета никогда не было.
    TokenVault::delete_refresh_token(&id)?;
    Ok(())
}

/// Добавляет Microsoft-аккаунт через Device Code Flow. `client_id` берётся
/// из настроек приложения (регистрация Azure-приложения). Прогресс идёт
/// через `Channel` — UI показывает код и ссылку для авторизации.
#[tauri::command]
#[specta::specta]
pub async fn account_add_microsoft(
    state: tauri::State<'_, AppState>,
    progress: Channel<AuthProgressDto>,
) -> Result<AccountDto> {
    // TODO: client_id должен браться из конфига или переменной окружения
    // После регистрации Azure-приложения заменить на реальный ID
    let client_id = std::env::var("DREAMLAUNCHER_CLIENT_ID")
        .unwrap_or_else(|_| "00000000-0000-0000-0000-000000000000".to_string());

    let result = authenticate_microsoft(&state.http, &client_id, |auth_progress| {
        let dto = match auth_progress {
            AuthProgress::WaitingForUser {
                user_code,
                verification_uri,
                expires_in,
            } => AuthProgressDto::WaitingForUser {
                user_code,
                verification_uri,
                expires_in: expires_in.min(u32::MAX as u64) as u32,
            },
            AuthProgress::Polling => AuthProgressDto::Polling,
            AuthProgress::ExchangingXboxLive => AuthProgressDto::ExchangingXboxLive,
            AuthProgress::ExchangingXsts => AuthProgressDto::ExchangingXsts,
            AuthProgress::LoggingIntoMinecraft => AuthProgressDto::LoggingIntoMinecraft,
            AuthProgress::FetchingProfile => AuthProgressDto::FetchingProfile,
            AuthProgress::Complete { uuid, username } => {
                AuthProgressDto::Complete { uuid, username }
            }
        };
        let _ = progress.send(dto);
    })
    .await;

    let auth_result = match result {
        Ok(r) => r,
        Err(e) => {
            let _ = progress.send(AuthProgressDto::Error {
                message: e.to_string(),
            });
            return Err(e.into());
        }
    };

    // Проверяем entitlements
    let has_license = check_minecraft_entitlements(&state.http, &auth_result.access_token)
        .await
        .unwrap_or(false);
    if !has_license {
        let msg = "Аккаунт не имеет лицензии Minecraft: Java Edition".to_string();
        let _ = progress.send(AuthProgressDto::Error {
            message: msg.clone(),
        });
        return Err(DreamError::Other(msg));
    }

    // Сохраняем refresh token в keyring
    TokenVault::store_refresh_token(&auth_result.uuid, &auth_result.refresh_token)?;

    // Записываем в БД
    let db = state.db.lock().expect("db mutex poisoned");
    let existing_active: i64 = db.query_row(
        "SELECT COUNT(*) FROM accounts WHERE is_active = 1",
        [],
        |r| r.get(0),
    )?;
    let is_active = existing_active == 0;

    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(auth_result.expires_in as i64);

    db.execute(
        "INSERT INTO accounts (id, username, kind, is_active, expires_at) VALUES (?1, ?2, 'microsoft', ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET username = excluded.username, expires_at = excluded.expires_at, is_active = excluded.is_active",
        rusqlite::params![auth_result.uuid, auth_result.username, is_active as i64, expires_at.to_rfc3339()],
    )?;

    Ok(AccountDto {
        id: auth_result.uuid,
        username: auth_result.username,
        kind: "microsoft".into(),
        is_active,
    })
}

/// Обновляет истёкший access token Microsoft-аккаунта через refresh token.
/// Автоматически вызывается при запуске инстанса, если токен протух.
#[tauri::command]
#[specta::specta]
pub async fn account_refresh_microsoft(
    state: tauri::State<'_, AppState>,
    account_id: String,
) -> Result<()> {
    let client_id = std::env::var("DREAMLAUNCHER_CLIENT_ID")
        .unwrap_or_else(|_| "00000000-0000-0000-0000-000000000000".to_string());

    let refresh_token = TokenVault::load_refresh_token(&account_id)?
        .ok_or_else(|| DreamError::Other(format!("для аккаунта {account_id} нет refresh token")))?;

    let new_token =
        dream_core::auth::refresh_microsoft_token(&state.http, &client_id, &refresh_token).await?;

    // Обновляем refresh token в keyring (он может измениться)
    TokenVault::store_refresh_token(&account_id, &new_token.refresh_token)?;

    // Обновляем expires_at в БД
    let db = state.db.lock().expect("db mutex poisoned");
    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(new_token.expires_in as i64);
    db.execute(
        "UPDATE accounts SET expires_at = ?1 WHERE id = ?2",
        rusqlite::params![expires_at.to_rfc3339(), account_id],
    )?;

    Ok(())
}
