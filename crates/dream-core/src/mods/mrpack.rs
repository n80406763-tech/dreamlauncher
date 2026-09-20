//! `.mrpack` — формат модпаков Modrinth: `modrinth.index.json` (список
//! файлов по хэшам + URL) плюс каталог `overrides/`, который распаковывается
//! как есть поверх `.minecraft/` (обрабатывает `download::archive::safe_extract`).

use crate::download::DownloadTask;
use crate::error::{CoreError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Скачивать файлы модпака разрешаем только с этих доменов — иначе
/// произвольный `.mrpack` превращается в вектор скачивания чего угодно
/// откуда угодно от лица пользователя лаунчера.
pub const ALLOWED_DOWNLOAD_HOSTS: &[&str] = &["cdn.modrinth.com", "github.com", "raw.githubusercontent.com"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHashes {
    pub sha1: String,
    pub sha512: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileEnv {
    #[serde(default)]
    pub client: Option<String>,
    #[serde(default)]
    pub server: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackFile {
    pub path: String,
    pub hashes: FileHashes,
    #[serde(default)]
    pub env: FileEnv,
    pub downloads: Vec<String>,
    pub file_size: u64,
}

impl PackFile {
    /// На стороне клиента не нужны файлы, помеченные `"unsupported"` для
    /// `client` (типичный случай — server-only плагины в модпаках,
    /// смешивающих клиентские и серверные моды).
    pub fn needed_on_client(&self) -> bool {
        self.env.client.as_deref() != Some("unsupported")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthIndex {
    pub format_version: u32,
    pub game: String,
    pub version_id: String,
    pub name: String,
    pub files: Vec<PackFile>,
    /// Обычно содержит `minecraft` и один из `fabric-loader` /
    /// `quilt-loader` / `forge` / `neoforge`.
    pub dependencies: HashMap<String, String>,
}

impl ModrinthIndex {
    pub fn parse(text: &str) -> Result<Self> {
        serde_json::from_str(text).map_err(|e| CoreError::json("modrinth.index.json", e))
    }

    pub fn loader_dependency(&self) -> Option<(&str, &str)> {
        for key in ["fabric-loader", "quilt-loader", "forge", "neoforge"] {
            if let Some(version) = self.dependencies.get(key) {
                return Some((key, version.as_str()));
            }
        }
        None
    }
}

fn host_is_allowed(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    matches!(parsed.host_str(), Some(host) if ALLOWED_DOWNLOAD_HOSTS.contains(&host))
}

/// Задачи на скачивание файлов модпака в `<instance>/.minecraft/<path>`.
/// Отклоняет весь пак (не отдельный файл), если хоть один URL указывает
/// на домен вне allowlist — молчаливо пропустить файл было бы хуже: пак
/// соберётся «рабочим», но без части контента, и это не будет замечено.
pub fn plan_downloads(index: &ModrinthIndex, game_dir: &Path) -> Result<Vec<DownloadTask>> {
    let mut tasks = Vec::with_capacity(index.files.len());
    for file in &index.files {
        if !file.needed_on_client() {
            continue;
        }
        let Some(url) = file.downloads.first() else {
            return Err(CoreError::Other(format!("файл {} в паке не содержит ни одной ссылки на скачивание", file.path)));
        };
        if !host_is_allowed(url) {
            return Err(CoreError::Other(format!("файл {} ссылается на недоверенный домен: {url}", file.path)));
        }
        tasks.push(DownloadTask {
            url: url.clone(),
            dest: game_dir.join(file.path.replace('/', std::path::MAIN_SEPARATOR_STR)),
            sha1: Some(file.hashes.sha1.clone()),
            size: Some(file.file_size),
        });
    }
    Ok(tasks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture() -> ModrinthIndex {
        let path = format!("{}/tests/fixtures/mrpack-index.json", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(path).unwrap();
        ModrinthIndex::parse(&text).unwrap()
    }

    #[test]
    fn parses_real_fabulously_optimized_index() {
        let index = fixture();
        assert_eq!(index.game, "minecraft");
        assert_eq!(index.loader_dependency(), Some(("fabric-loader", "0.19.3")));
        assert_eq!(index.dependencies.get("minecraft").map(String::as_str), Some("26.2"));
        assert_eq!(index.files.len(), 3);
    }

    #[test]
    fn plans_downloads_for_allowed_hosts() {
        let index = fixture();
        let tasks = plan_downloads(&index, &PathBuf::from(r"C:\instances\demo\.minecraft")).unwrap();
        assert_eq!(tasks.len(), 3);
        assert!(tasks[0].dest.ends_with("mods/BetterGrassify-1.8.7+fabric.26.2.jar") || tasks[0].dest.to_string_lossy().contains("BetterGrassify"));
        assert_eq!(tasks[0].sha1.as_deref(), Some("0f4a890d07402280686a518579fa9fa02309e315"));
    }

    #[test]
    fn rejects_pack_with_untrusted_download_host() {
        let mut index = fixture();
        index.files[0].downloads = vec!["https://evil.example.com/payload.jar".to_string()];
        let result = plan_downloads(&index, &PathBuf::from(r"C:\instances\demo\.minecraft"));
        assert!(result.is_err());
    }

    #[test]
    fn skips_files_marked_unsupported_on_client() {
        let mut index = fixture();
        index.files[0].env.client = Some("unsupported".to_string());
        let tasks = plan_downloads(&index, &PathBuf::from(r"C:\instances\demo\.minecraft")).unwrap();
        assert_eq!(tasks.len(), 2);
    }
}
