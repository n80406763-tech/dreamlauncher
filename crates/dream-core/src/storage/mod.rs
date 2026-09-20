//! SQLite-хранилище метаданных (инстансы, аккаунты без секретов,
//! установленные моды, настройки). CRUD-обёртки поверх схемы — M4.

pub mod paths;
pub mod schema;

pub use paths::AppPaths;
pub use schema::open;
