//! Метаданные версий: манифест Mojang, разбор `<version>.json`, слияние
//! цепочки `inheritsFrom` и оценка `rules`.

pub mod assets;
pub mod manifest;
pub mod resolve;
pub mod rules;
pub mod version_json;

pub use assets::{AssetIndex, AssetLayout};
pub use manifest::{fetch_version_json, fetch_version_manifest, VersionManifest, VersionManifestEntry};
pub use resolve::{merge_chain, ResolvedVersion};
pub use rules::RuleContext;
pub use version_json::RawVersionJson;
