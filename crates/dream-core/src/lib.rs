//! `dream-core` — движок DreamLauncher: метаданные версий, загрузка
//! файлов, Java, загрузчики модов, сборка команды запуска, моды.
//! Не зависит от Tauri — тестируется обычным `cargo test`, а IPC-слой
//! в `src-tauri` лишь оборачивает эти функции в команды.

pub mod auth;
pub mod download;
pub mod error;
pub mod install;
pub mod java;
pub mod launch;
pub mod loaders;
pub mod meta;
pub mod mods;
pub mod shaders;
pub mod storage;

pub use error::{CoreError, Result};
