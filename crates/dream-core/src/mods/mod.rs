//! Моды: поиск/установка через Modrinth и импорт `.mrpack`-модпаков.
//! CurseForge-каталог за фиче-флагом (нет ключа API) — см. план, M7.

pub mod modrinth;
pub mod mrpack;

pub use modrinth::{search, DependencyType, ProjectVersion, SearchResponse};
pub use mrpack::ModrinthIndex;
