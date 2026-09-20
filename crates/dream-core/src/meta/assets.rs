//! Индекс ассетов (`assets/indexes/<id>.json`) и планирование того, как
//! разложить объекты на диске: по content-hash (современные версии),
//! деревом `virtual/legacy` (≤1.7.2) или прямо в `<gameDir>/resources`
//! (≤1.5.2, флаг `map_to_resources`).

use crate::download::DownloadTask;
use crate::error::{CoreError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const RESOURCES_BASE_URL: &str = "https://resources.download.minecraft.net";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
    #[serde(rename = "virtual", default)]
    pub is_virtual: bool,
    #[serde(default)]
    pub map_to_resources: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetLayout {
    /// Игра сама читает объекты по хэшу из `assets/objects/` — раскладка не нужна.
    Modern,
    /// Собрать дерево `assets/virtual/legacy/<путь>` из объектов по хэшу.
    VirtualLegacy,
    /// Разложить прямо в `<gameDir>/resources/<путь>`.
    MapToResources,
}

impl AssetIndex {
    pub fn parse(text: &str) -> Result<Self> {
        serde_json::from_str(text).map_err(|e| CoreError::json("asset index", e))
    }

    pub fn layout(&self) -> AssetLayout {
        if self.map_to_resources {
            AssetLayout::MapToResources
        } else if self.is_virtual {
            AssetLayout::VirtualLegacy
        } else {
            AssetLayout::Modern
        }
    }

    /// Путь объекта относительно `assets/objects/`, например `bd/bdf48ef6...`.
    pub fn object_relative_path(hash: &str) -> PathBuf {
        PathBuf::from(&hash[0..2]).join(hash)
    }

    /// Задачи на скачивание всех объектов индекса в `objects_dir`
    /// (обычно `shared/assets/objects`).
    pub fn download_tasks(&self, objects_dir: &Path) -> Vec<DownloadTask> {
        self.objects
            .values()
            .map(|obj| DownloadTask {
                url: format!("{RESOURCES_BASE_URL}/{}/{}", &obj.hash[0..2], obj.hash),
                dest: objects_dir.join(Self::object_relative_path(&obj.hash)),
                sha1: Some(obj.hash.clone()),
                size: Some(obj.size),
            })
            .collect()
    }

    /// Для `VirtualLegacy`/`MapToResources`: пары (источник в `objects_dir`,
    /// куда положить копию) — источник уже должен быть скачан
    /// `download_tasks`. Вызывающий код сам решает, копировать файл или
    /// делать хардлинк.
    pub fn legacy_copy_plan(&self, objects_dir: &Path, target_dir: &Path) -> Vec<(PathBuf, PathBuf)> {
        self.objects
            .iter()
            .map(|(asset_path, obj)| {
                let source = objects_dir.join(Self::object_relative_path(&obj.hash));
                let dest = target_dir.join(asset_path.replace('/', std::path::MAIN_SEPARATOR_STR));
                (source, dest)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> AssetIndex {
        let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(path).unwrap();
        AssetIndex::parse(&text).unwrap()
    }

    #[test]
    fn modern_index_has_no_special_layout() {
        let index = fixture("asset-index-modern.json");
        assert_eq!(index.layout(), AssetLayout::Modern);
        assert!(!index.objects.is_empty());

        let tasks = index.download_tasks(Path::new(r"C:\shared\assets\objects"));
        let sample = &tasks[0];
        assert!(sample.url.starts_with(RESOURCES_BASE_URL));
        assert!(sample.sha1.is_some());
    }

    #[test]
    fn legacy_index_is_virtual_and_produces_copy_plan() {
        let index = fixture("asset-index-legacy.json");
        assert_eq!(index.layout(), AssetLayout::VirtualLegacy);

        let plan = index.legacy_copy_plan(
            Path::new(r"C:\shared\assets\objects"),
            Path::new(r"C:\shared\assets\virtual\legacy"),
        );
        assert_eq!(plan.len(), index.objects.len());
        let readme = plan.iter().find(|(_, dest)| dest.ends_with("READ_ME_I_AM_VERY_IMPORTANT.txt"));
        assert!(readme.is_some());
    }

    #[test]
    fn object_relative_path_uses_first_two_hash_chars_as_bucket() {
        let path = AssetIndex::object_relative_path("bdf48ef6b5d0d23bbb02e17d04865216179f510a");
        assert_eq!(path, PathBuf::from("bd").join("bdf48ef6b5d0d23bbb02e17d04865216179f510a"));
    }
}
