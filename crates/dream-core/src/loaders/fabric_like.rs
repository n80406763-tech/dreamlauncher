//! Fabric и Quilt отдают уже готовый `<version>.json` (с `inheritsFrom`)
//! через один и тот же формат meta-API — поэтому одна реализация
//! обслуживает оба загрузчика, различается только базовый URL.

use crate::error::{CoreError, Result};
use crate::meta::RawVersionJson;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FabricLikeKind {
    Fabric,
    Quilt,
}

impl FabricLikeKind {
    fn base_url(self) -> &'static str {
        match self {
            FabricLikeKind::Fabric => "https://meta.fabricmc.net/v2",
            FabricLikeKind::Quilt => "https://meta.quiltmc.org/v3",
        }
    }

    pub fn loader_slug(self) -> &'static str {
        match self {
            FabricLikeKind::Fabric => "fabric",
            FabricLikeKind::Quilt => "quilt",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoaderBuild {
    pub separator: Option<String>,
    pub build: Option<u32>,
    pub maven: String,
    pub version: String,
    pub stable: bool,
}

pub fn loader_versions_url(kind: FabricLikeKind, mc_version: &str) -> String {
    format!("{}/versions/loader/{mc_version}", kind.base_url())
}

pub fn profile_json_url(kind: FabricLikeKind, mc_version: &str, loader_version: &str) -> String {
    format!("{}/versions/loader/{mc_version}/{loader_version}/profile/json", kind.base_url())
}

/// Список версий загрузчика, доступных для данной версии Minecraft
/// (первый элемент — самый свежий, `stable` отмечает рекомендуемые сборки).
pub async fn fetch_loader_versions(client: &reqwest::Client, kind: FabricLikeKind, mc_version: &str) -> Result<Vec<LoaderBuild>> {
    let url = loader_versions_url(kind, mc_version);
    let text = client.get(&url).send().await?.error_for_status()?.text().await?;
    serde_json::from_str(&text).map_err(|e| CoreError::json(url, e))
}

/// Готовый `<version>.json` с `inheritsFrom: <mc_version>` — сразу
/// пригоден для `meta::merge_chain` вместе с ванильным родителем.
pub async fn fetch_profile_json(client: &reqwest::Client, kind: FabricLikeKind, mc_version: &str, loader_version: &str) -> Result<RawVersionJson> {
    let url = profile_json_url(kind, mc_version, loader_version);
    let text = client.get(&url).send().await?.error_for_status()?.text().await?;
    serde_json::from_str(&text).map_err(|e| CoreError::json(url, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_correct_urls_per_loader() {
        assert_eq!(loader_versions_url(FabricLikeKind::Fabric, "1.21.1"), "https://meta.fabricmc.net/v2/versions/loader/1.21.1");
        assert_eq!(loader_versions_url(FabricLikeKind::Quilt, "1.21.1"), "https://meta.quiltmc.org/v3/versions/loader/1.21.1");
        assert_eq!(
            profile_json_url(FabricLikeKind::Fabric, "1.21.1", "0.16.9"),
            "https://meta.fabricmc.net/v2/versions/loader/1.21.1/0.16.9/profile/json"
        );
    }

    #[test]
    fn parses_real_fabric_profile_fixture_as_raw_version_json() {
        let path = format!("{}/tests/fixtures/fabric-1.21.1.json", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(path).unwrap();
        let parsed: RawVersionJson = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed.inherits_from.as_deref(), Some("1.21.1"));
        assert!(parsed.main_class.contains("fabricmc"));
    }
}
