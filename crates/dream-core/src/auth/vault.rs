//! Хранилище секретов аккаунтов (refresh token для MSA) — только
//! Windows Credential Manager через `keyring`, никогда файл на диске.
//! В SQLite (`storage`) хранится лишь ссылка на запись здесь: `uuid`,
//! `username`, `expires_at`.

use crate::error::{CoreError, Result};

const SERVICE_NAME: &str = "DreamLauncher";

pub struct TokenVault;

impl TokenVault {
    /// `account_id` — обычно UUID аккаунта Minecraft; один секрет на аккаунт.
    fn entry(account_id: &str) -> Result<keyring::Entry> {
        keyring::Entry::new(SERVICE_NAME, account_id).map_err(|e| CoreError::Auth(format!("не удалось открыть хранилище секретов: {e}")))
    }

    pub fn store_refresh_token(account_id: &str, refresh_token: &str) -> Result<()> {
        Self::entry(account_id)?.set_password(refresh_token).map_err(|e| CoreError::Auth(format!("не удалось сохранить токен: {e}")))
    }

    /// `None`, если для аккаунта ещё ничего не сохранено — это не ошибка.
    pub fn load_refresh_token(account_id: &str) -> Result<Option<String>> {
        match Self::entry(account_id)?.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(CoreError::Auth(format!("не удалось прочитать токен: {e}"))),
        }
    }

    pub fn delete_refresh_token(account_id: &str) -> Result<()> {
        match Self::entry(account_id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(CoreError::Auth(format!("не удалось удалить токен: {e}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Использует реальный Windows Credential Manager — идёт под уникальным
    // тестовым account_id и подчищает за собой, чтобы не оставить мусор.
    #[test]
    fn round_trips_through_windows_credential_manager() {
        let account_id = format!("dream-core-test-{}", uuid::Uuid::new_v4());

        assert_eq!(TokenVault::load_refresh_token(&account_id).unwrap(), None);

        TokenVault::store_refresh_token(&account_id, "super-secret-refresh-token").unwrap();
        assert_eq!(TokenVault::load_refresh_token(&account_id).unwrap().as_deref(), Some("super-secret-refresh-token"));

        TokenVault::delete_refresh_token(&account_id).unwrap();
        assert_eq!(TokenVault::load_refresh_token(&account_id).unwrap(), None);
    }
}
