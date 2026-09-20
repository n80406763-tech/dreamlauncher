//! `install_profile.json`, извлекаемый из installer-jar Forge/NeoForge.
//! Формат совпадает для обоих загрузчиков — NeoForge является форком
//! Forge и унаследовал его установочный пайплайн.

use crate::meta::version_json::Library;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataEntry {
    pub client: String,
    pub server: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProcessorEntry {
    /// `None` значит «применяется к обеим сторонам» — сериализуется как
    /// отсутствие поля в оригинальном JSON.
    #[serde(default)]
    pub sides: Option<Vec<String>>,
    pub jar: String,
    #[serde(default)]
    pub classpath: Vec<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub outputs: HashMap<String, String>,
}

impl ProcessorEntry {
    pub fn applies_to_client(&self) -> bool {
        match &self.sides {
            None => true,
            Some(sides) => sides.iter().any(|s| s == "client"),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallProfile {
    pub spec: u32,
    pub profile: String,
    pub version: String,
    pub minecraft: String,
    #[serde(default)]
    pub data: HashMap<String, DataEntry>,
    #[serde(default)]
    pub processors: Vec<ProcessorEntry>,
    #[serde(default)]
    pub libraries: Vec<Library>,
}

impl InstallProfile {
    pub fn parse(text: &str) -> crate::error::Result<Self> {
        serde_json::from_str(text).map_err(|e| crate::error::CoreError::json("install_profile.json", e))
    }

    pub fn client_processors(&self) -> impl Iterator<Item = &ProcessorEntry> {
        self.processors.iter().filter(|p| p.applies_to_client())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_text() -> String {
        let path = format!("{}/tests/fixtures/forge-1.20.1-install_profile.json", env!("CARGO_MANIFEST_DIR"));
        std::fs::read_to_string(path).unwrap()
    }

    #[test]
    fn parses_real_forge_install_profile() {
        let profile = InstallProfile::parse(&fixture_text()).unwrap();
        assert_eq!(profile.minecraft, "1.20.1");
        assert!(profile.data.contains_key("BINPATCH"));
        assert!(!profile.processors.is_empty());
        assert!(!profile.libraries.is_empty());
    }

    #[test]
    fn filters_processors_applicable_to_client() {
        let profile = InstallProfile::parse(&fixture_text()).unwrap();
        let client: Vec<_> = profile.client_processors().collect();
        // Ни один процессор, помеченный только "server", не должен пройти фильтр.
        assert!(client.iter().all(|p| p.sides.as_ref().is_none_or(|s| s.iter().any(|x| x == "client"))));
        // Но хотя бы один действительно клиентский (jarsplitter) должен остаться.
        assert!(client.iter().any(|p| p.jar.contains("jarsplitter")));
    }
}
