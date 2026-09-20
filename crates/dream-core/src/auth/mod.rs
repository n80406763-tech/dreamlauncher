//! Аккаунты: офлайн-профили (готово) и Microsoft OAuth (типы, эндпоинты,
//! и полная сетевая реализация), плюс хранилище секретов в Windows Credential Manager.

pub mod microsoft;
pub mod microsoft_flow;
pub mod offline;
pub mod vault;

pub use microsoft_flow::{authenticate_microsoft, check_minecraft_entitlements, refresh_microsoft_token, AuthProgress, AuthResult};
pub use offline::{is_valid_player_name, offline_uuid};
pub use vault::TokenVault;
