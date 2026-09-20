//! Установка управляемого Java-рантайма Mojang на диск: разбор
//! `manifest.json` компонента (см. `runtime::RuntimeEntry::manifest`) и
//! планирование задач на скачивание + список каталогов/исполняемых файлов.
//!
//! Реальная запись битов "исполняемый" (`chmod +x`) не нужна на Windows —
//! единственной целевой платформе проекта (см. план) — поэтому здесь
//! только план, а не `std::os::unix`-специфичный код.

use crate::download::DownloadTask;
use crate::error::{CoreError, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeRawDownload {
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeFileDownloads {
    pub raw: RuntimeRawDownload,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeManifestEntry {
    #[serde(rename = "type")]
    pub kind: String, // "file" | "directory" | "link"
    #[serde(default)]
    pub executable: bool,
    #[serde(default)]
    pub downloads: Option<RuntimeFileDownloads>,
}

/// `manifest.json` конкретного компонента/платформы — плоский список всех
/// файлов и каталогов рантайма с относительными путями в качестве ключей.
#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeFilesManifest {
    pub files: HashMap<String, RuntimeManifestEntry>,
}

impl RuntimeFilesManifest {
    pub fn parse(text: &str) -> Result<Self> {
        serde_json::from_str(text).map_err(|e| CoreError::json("java runtime manifest.json", e))
    }

    /// Каталоги, которые нужно создать до скачивания файлов (иначе
    /// конкурентная запись файлов в ещё не созданный каталог упадёт).
    pub fn directories(&self, java_home: &Path) -> Vec<PathBuf> {
        self.files
            .iter()
            .filter(|(_, e)| e.kind == "directory")
            .map(|(path, _)| java_home.join(path.replace('/', std::path::MAIN_SEPARATOR_STR)))
            .collect()
    }

    /// Задачи на скачивание всех файлов рантайма (сырые, не lzma —
    /// `raw`-загрузка проще и её объём для JRE не критичен).
    pub fn download_tasks(&self, java_home: &Path) -> Vec<DownloadTask> {
        self.files
            .iter()
            .filter_map(|(path, e)| {
                if e.kind != "file" {
                    return None;
                }
                let dl = e.downloads.as_ref()?;
                Some(DownloadTask {
                    url: dl.raw.url.clone(),
                    dest: java_home.join(path.replace('/', std::path::MAIN_SEPARATOR_STR)),
                    sha1: Some(dl.raw.sha1.clone()),
                    size: Some(dl.raw.size),
                })
            })
            .collect()
    }
}

/// Путь к `javaw.exe` (без консольного окна) в уже установленном рантайме,
/// с фолбэком на `java.exe`, если по какой-то причине `javaw` отсутствует.
pub fn java_executable_path(java_home: &Path) -> PathBuf {
    let javaw = java_home.join("bin").join("javaw.exe");
    if javaw.is_file() {
        javaw
    } else {
        java_home.join("bin").join("java.exe")
    }
}

/// Рантайм уже установлен, если в нём есть `bin/java.exe` — простая и
/// достаточная проверка (частичная/битая загрузка крайне маловероятна
/// благодаря sha1-проверке в движке загрузки, который либо докачивает
/// файл целиком, либо оставляет его отсутствующим).
pub fn is_installed(java_home: &Path) -> bool {
    java_home.join("bin").join("java.exe").is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> RuntimeFilesManifest {
        let path = format!("{}/tests/fixtures/java-runtime-manifest.json", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(path).unwrap();
        RuntimeFilesManifest::parse(&text).unwrap()
    }

    #[test]
    fn plans_directories_and_downloads_from_real_manifest() {
        let manifest = fixture();
        let home = Path::new(r"C:\shared\java\java-runtime-delta");

        let dirs = manifest.directories(home);
        assert!(dirs.contains(&home.join("bin")));
        assert!(dirs.contains(&home.join("bin").join("server")));

        let tasks = manifest.download_tasks(home);
        // 4 файла с downloads в фикстуре: java.exe, javaw.exe, dll, license.
        assert_eq!(tasks.len(), 4);
        let java_exe = tasks.iter().find(|t| t.dest.ends_with("bin\\java.exe") || t.dest.ends_with("bin/java.exe")).unwrap();
        assert!(java_exe.sha1.is_some());
        assert_eq!(java_exe.size, Some(39424));
    }

    #[test]
    fn detects_installed_runtime_by_java_exe_presence() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_installed(dir.path()));

        std::fs::create_dir_all(dir.path().join("bin")).unwrap();
        std::fs::write(dir.path().join("bin").join("java.exe"), b"fake").unwrap();
        assert!(is_installed(dir.path()));
    }

    #[test]
    fn prefers_javaw_over_java_when_both_present() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("bin")).unwrap();
        std::fs::write(dir.path().join("bin").join("java.exe"), b"fake").unwrap();
        assert_eq!(java_executable_path(dir.path()), dir.path().join("bin").join("java.exe"));

        std::fs::write(dir.path().join("bin").join("javaw.exe"), b"fake").unwrap();
        assert_eq!(java_executable_path(dir.path()), dir.path().join("bin").join("javaw.exe"));
    }
}
