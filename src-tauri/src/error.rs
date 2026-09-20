//! Ошибка, пересекающая границу IPC. Оборачивает `dream_core::CoreError` и
//! `rusqlite`/`std::io` в единый сериализуемый тип, который `tauri-specta`
//! превращает в размеченный TS-тип на фронтенде.

use serde::Serialize;
use specta::Type;

#[derive(Debug, Clone, Serialize, Type, thiserror::Error)]
#[serde(tag = "kind", content = "message")]
pub enum DreamError {
    #[error("{0}")]
    Core(String),
    #[error("{0}")]
    Io(String),
    #[error("{0}")]
    Other(String),
}

impl From<dream_core::CoreError> for DreamError {
    fn from(err: dream_core::CoreError) -> Self {
        DreamError::Core(err.to_string())
    }
}

impl From<std::io::Error> for DreamError {
    fn from(err: std::io::Error) -> Self {
        DreamError::Io(err.to_string())
    }
}

impl From<rusqlite::Error> for DreamError {
    fn from(err: rusqlite::Error) -> Self {
        DreamError::Other(format!("SQLite: {err}"))
    }
}

pub type Result<T> = std::result::Result<T, DreamError>;
