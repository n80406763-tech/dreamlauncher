//! Единый тип ошибки для всего ядра. Всё, что уходит через IPC во фронтенд,
//! в итоге сериализуется в этот тип (см. `DreamError` в `src-tauri`).

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("сетевой запрос не удался: {0}")]
    Network(#[from] reqwest::Error),

    #[error("ошибка ввода-вывода при работе с {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("не удалось разобрать JSON ({context}): {source}")]
    Json {
        context: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("проверка контрольной суммы не пройдена для {path}: ожидалось {expected}, получено {actual}")]
    ChecksumMismatch {
        path: String,
        expected: String,
        actual: String,
    },

    #[error("версия {0} не найдена в манифесте")]
    VersionNotFound(String),

    #[error("загрузчик {loader} не поддерживает версию {mc_version}")]
    UnsupportedLoader { loader: String, mc_version: String },

    #[error("не найдена подходящая Java для компонента {component}")]
    JavaNotFound { component: String },

    #[error("ошибка авторизации: {0}")]
    Auth(String),

    #[error("хранилище (SQLite) недоступно: {0}")]
    Storage(#[from] rusqlite::Error),

    #[error("небезопасный путь в архиве (zip-slip): {0}")]
    UnsafeArchivePath(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;

impl CoreError {
    pub fn io(path: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io { path: path.into(), source }
    }

    pub fn json(context: impl Into<String>, source: serde_json::Error) -> Self {
        Self::Json { context: context.into(), source }
    }
}
