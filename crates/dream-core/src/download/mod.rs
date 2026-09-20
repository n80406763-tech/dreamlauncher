//! Скачивание и безопасная распаковка файлов: общий движок с проверкой
//! sha1, атомарной записью и защитой от zip-slip.

pub mod archive;
pub mod engine;
pub mod hash;

pub use archive::{safe_extract, ExtractOptions};
pub use engine::{download_all, DownloadEvent, DownloadTask, DownloaderConfig};
pub use hash::{sha1_hex, sha1_hex_file};
