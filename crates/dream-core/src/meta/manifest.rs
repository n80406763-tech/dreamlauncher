//! Манифест всех версий Minecraft (`version_manifest_v2.json`) и загрузка
//! отдельных `<version>.json` по URL из него.

use super::version_json::RawVersionJson;
use crate::error::{CoreError, Result};
use serde::{Deserialize, Serialize};

pub const VERSION_MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionKind {
    Release,
    Snapshot,
    OldBeta,
    OldAlpha,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionManifestEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: VersionKind,
    pub url: String,
    pub time: String,
    pub release_time: String,
    pub sha1: String,
    #[serde(default)]
    pub compliance_level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionManifest {
    pub latest: LatestVersions,
    pub versions: Vec<VersionManifestEntry>,
}

impl VersionManifest {
    pub fn find(&self, id: &str) -> Option<&VersionManifestEntry> {
        self.versions.iter().find(|v| v.id == id)
    }

    pub fn parse(text: &str) -> Result<Self> {
        serde_json::from_str(text).map_err(|e| CoreError::json("version_manifest_v2.json", e))
    }
}

/// Скачивает и разбирает манифест версий с Mojang.
pub async fn fetch_version_manifest(client: &reqwest::Client) -> Result<VersionManifest> {
    let text = client.get(VERSION_MANIFEST_URL).send().await?.error_for_status()?.text().await?;
    VersionManifest::parse(&text)
}

/// Скачивает и разбирает конкретный `<version>.json` по URL из манифеста.
pub async fn fetch_version_json(client: &reqwest::Client, url: &str) -> Result<RawVersionJson> {
    let text = client.get(url).send().await?.error_for_status()?.text().await?;
    serde_json::from_str(&text).map_err(|e| CoreError::json(url.to_string(), e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_trimmed_manifest_fixture() {
        let path = format!("{}/tests/fixtures/version_manifest_v2.json", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(path).unwrap();
        let manifest = VersionManifest::parse(&text).unwrap();

        assert_eq!(manifest.latest.release, "26.2");
        assert!(manifest.find("1.21.1").is_some());
        assert!(manifest.find("1.6.4").is_some());
        assert!(manifest.find("does-not-exist").is_none());

        let old = manifest.find("1.6.4").unwrap();
        assert_eq!(old.kind, VersionKind::Release);
    }
}
