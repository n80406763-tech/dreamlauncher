//! Общее состояние приложения: HTTP-клиент, пути на диске, соединение с
//! SQLite. Живёт в `tauri::State`, доступно любой команде.

use dream_core::storage::AppPaths;
use std::sync::Mutex;

pub struct AppState {
    pub http: reqwest::Client,
    pub paths: AppPaths,
    pub db: Mutex<rusqlite::Connection>,
}

impl AppState {
    pub fn init() -> dream_core::Result<Self> {
        let root = AppPaths::default_root()
            .ok_or_else(|| dream_core::CoreError::Other("не удалось определить каталог данных приложения (%APPDATA%)".into()))?;
        let paths = AppPaths::new(root);
        paths.ensure_shared_dirs().map_err(|e| dream_core::CoreError::io(paths.root.display().to_string(), e))?;
        let db = dream_core::storage::open(&paths.db_path())?;

        let http = reqwest::Client::builder()
            .user_agent(dream_core::mods::modrinth::user_agent(env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(dream_core::CoreError::Network)?;

        Ok(Self { http, paths, db: Mutex::new(db) })
    }
}
