//! Forge и NeoForge: разбор `install_profile.json`, резолвер `data`/
//! `{TOKEN}` и полное выполнение процессоров патчинга (`run`) — то, что
//! запускает пропатченный клиент из своего собственного jar-архива, а не
//! качает его напрямую по URL.

pub mod data_resolver;
pub mod install_profile;
pub mod installer;
pub mod run;

pub use data_resolver::{parse_data_value, substitute, DataValue, ProcessorContext};
pub use install_profile::{InstallProfile, ProcessorEntry};
pub use installer::{read_main_class, read_zip_entry_bytes, read_zip_entry_text};
pub use run::{run_client_processors, ForgeInstallContext};

use crate::error::{CoreError, Result};
use serde::Deserialize;

pub const FORGE_ALL_VERSIONS_METADATA_URL: &str = "https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json";
pub const NEOFORGE_MAVEN_METADATA_URL: &str = "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";

pub fn forge_installer_url(mc_version: &str, forge_version: &str) -> String {
    format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{mc_version}-{forge_version}/forge-{mc_version}-{forge_version}-installer.jar")
}

pub fn neoforge_installer_url(neoforge_version: &str) -> String {
    format!("https://maven.neoforged.net/releases/net/neoforged/neoforge/{neoforge_version}/neoforge-{neoforge_version}-installer.jar")
}

/// Список версий Forge для конкретной версии Minecraft, новые первыми.
/// Forge не отдаёт версии по одной — только весь `maven-metadata.json`
/// разом (`{"<mc_version>": ["<mc_version>-<forge_version>", ...]}`).
pub async fn fetch_forge_versions(client: &reqwest::Client, mc_version: &str) -> Result<Vec<String>> {
    let text = client.get(FORGE_ALL_VERSIONS_METADATA_URL).send().await?.error_for_status()?.text().await?;
    let all: std::collections::HashMap<String, Vec<String>> = serde_json::from_str(&text).map_err(|e| CoreError::json("forge maven-metadata.json", e))?;
    let prefix = format!("{mc_version}-");
    let mut versions: Vec<String> = all.get(mc_version).into_iter().flatten().filter_map(|full| full.strip_prefix(&prefix).map(str::to_string)).collect();
    versions.reverse(); // в источнике от старых к новым
    Ok(versions)
}

#[derive(Debug, Deserialize)]
struct NeoForgeMetadata {
    versioning: NeoForgeVersioning,
}
#[derive(Debug, Deserialize)]
struct NeoForgeVersioning {
    versions: NeoForgeVersions,
}
#[derive(Debug, Deserialize)]
struct NeoForgeVersions {
    #[serde(rename = "version", default)]
    version: Vec<String>,
}

/// NeoForge использует собственную нумерацию, совпадающую с версией
/// Minecraft без ведущего `1.` (1.20.1 → `20.1.*`, 1.21 → `21.0.*`) —
/// эвристика, не гарантия для всех будущих схем версионирования.
fn neoforge_version_prefix(mc_version: &str) -> String {
    let stripped = mc_version.strip_prefix("1.").unwrap_or(mc_version);
    if stripped.contains('.') {
        format!("{stripped}.")
    } else {
        format!("{stripped}.0.")
    }
}

pub async fn fetch_neoforge_versions(client: &reqwest::Client, mc_version: &str) -> Result<Vec<String>> {
    let text = client.get(NEOFORGE_MAVEN_METADATA_URL).send().await?.error_for_status()?.text().await?;
    let metadata: NeoForgeMetadata = quick_xml::de::from_str(&text).map_err(|e| CoreError::Other(format!("не удалось разобрать maven-metadata.xml NeoForge: {e}")))?;
    let prefix = neoforge_version_prefix(mc_version);
    let mut versions: Vec<String> = metadata.versioning.versions.version.into_iter().filter(|v| v.starts_with(&prefix)).collect();
    versions.reverse();
    Ok(versions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_forge_installer_url() {
        assert_eq!(
            forge_installer_url("1.20.1", "47.4.0"),
            "https://maven.minecraftforge.net/net/minecraftforge/forge/1.20.1-47.4.0/forge-1.20.1-47.4.0-installer.jar"
        );
    }

    #[test]
    fn builds_neoforge_installer_url() {
        assert_eq!(
            neoforge_installer_url("21.1.87"),
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/21.1.87/neoforge-21.1.87-installer.jar"
        );
    }

    #[test]
    fn neoforge_prefix_handles_two_and_three_segment_versions() {
        assert_eq!(neoforge_version_prefix("1.20.1"), "20.1.");
        assert_eq!(neoforge_version_prefix("1.21"), "21.0.");
    }

    #[test]
    fn parses_real_neoforge_maven_metadata_fixture() {
        let path = format!("{}/tests/fixtures/neoforge-maven-metadata.xml", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(path).unwrap();
        let metadata: NeoForgeMetadata = quick_xml::de::from_str(&text).unwrap();
        assert!(metadata.versioning.versions.version.iter().any(|v| v.starts_with("21.1.")));
        assert_eq!(metadata.versioning.versions.version.len(), 5);
    }

    #[test]
    fn filters_fixture_versions_by_mc_version_prefix() {
        let path = format!("{}/tests/fixtures/neoforge-maven-metadata.xml", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(path).unwrap();
        let metadata: NeoForgeMetadata = quick_xml::de::from_str(&text).unwrap();
        let prefix = neoforge_version_prefix("1.21.1");
        let versions: Vec<_> = metadata.versioning.versions.version.into_iter().filter(|v| v.starts_with(&prefix)).collect();
        assert_eq!(versions, vec!["21.1.1".to_string(), "21.1.2".to_string(), "21.1.87".to_string()]);
    }
}
