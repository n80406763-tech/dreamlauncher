//! Манифест управляемых Java-рантаймов Mojang
//! (`java-runtime/.../all.json`) — источник Java, которую лаунчер ставит
//! сам, без участия пользователя.

use crate::error::{CoreError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const JAVA_RUNTIME_MANIFEST_URL: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeVersion {
    pub name: String,
    pub released: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeManifestRef {
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeAvailability {
    pub group: u32,
    pub progress: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeEntry {
    pub availability: RuntimeAvailability,
    pub manifest: RuntimeManifestRef,
    pub version: RuntimeVersion,
}

/// `all.json` целиком: платформа -> компонент -> список вариантов
/// (на практике всегда один элемент).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JavaRuntimeManifest(pub HashMap<String, HashMap<String, Vec<RuntimeEntry>>>);

impl JavaRuntimeManifest {
    pub fn parse(text: &str) -> Result<Self> {
        serde_json::from_str(text).map_err(|e| CoreError::json("java-runtime all.json", e))
    }

    pub fn entry(&self, platform: &str, component: &str) -> Option<&RuntimeEntry> {
        self.0.get(platform)?.get(component)?.first()
    }
}

/// Ключ платформы, под которым Mojang публикует рантаймы, для текущей ОС.
pub fn current_platform_key() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => "windows-x64",
        ("windows", "x86") => "windows-x86",
        ("windows", "aarch64") => "windows-arm64",
        ("macos", "aarch64") => "mac-os-arm64",
        ("macos", _) => "mac-os",
        (_, "x86") => "linux-i386",
        _ => "linux",
    }
}

pub async fn fetch_java_runtime_manifest(client: &reqwest::Client) -> Result<JavaRuntimeManifest> {
    let text = client.get(JAVA_RUNTIME_MANIFEST_URL).send().await?.error_for_status()?.text().await?;
    JavaRuntimeManifest::parse(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> JavaRuntimeManifest {
        let path = format!("{}/tests/fixtures/java_runtime_all.json", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(path).unwrap();
        JavaRuntimeManifest::parse(&text).unwrap()
    }

    #[test]
    fn finds_known_components_for_windows_x64() {
        let manifest = fixture();
        let jre_legacy = manifest.entry("windows-x64", "jre-legacy").unwrap();
        assert!(jre_legacy.version.name.starts_with("8u"));

        let delta = manifest.entry("windows-x64", "java-runtime-delta").unwrap();
        assert!(delta.version.name.starts_with("21."));

        assert!(manifest.entry("windows-x64", "does-not-exist").is_none());
    }
}
