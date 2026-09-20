//! Раскладка данных лаунчера на диске (см. план, раздел 1): общие
//! версии/библиотеки/ассеты/Java в `shared/`, у каждого инстанса — только
//! свой `.minecraft/`.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub root: PathBuf,
}

impl AppPaths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// `%APPDATA%\DreamLauncher` на Windows (`dirs::data_dir()` даёт этот
    /// путь на любой ОС по её собственной конвенции).
    pub fn default_root() -> Option<PathBuf> {
        dirs::data_dir().map(|d| d.join("DreamLauncher"))
    }

    pub fn db_path(&self) -> PathBuf {
        self.root.join("launcher.db")
    }

    pub fn settings_path(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    pub fn shared_dir(&self) -> PathBuf {
        self.root.join("shared")
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.shared_dir().join("versions")
    }

    pub fn libraries_dir(&self) -> PathBuf {
        self.shared_dir().join("libraries")
    }

    pub fn assets_dir(&self) -> PathBuf {
        self.shared_dir().join("assets")
    }

    pub fn assets_objects_dir(&self) -> PathBuf {
        self.assets_dir().join("objects")
    }

    pub fn java_dir(&self) -> PathBuf {
        self.shared_dir().join("java")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.shared_dir().join("cache")
    }

    pub fn instances_dir(&self) -> PathBuf {
        self.root.join("instances")
    }

    pub fn instance_dir(&self, slug: &str) -> PathBuf {
        self.instances_dir().join(slug)
    }

    pub fn instance_game_dir(&self, slug: &str) -> PathBuf {
        self.instance_dir(slug).join(".minecraft")
    }

    /// Создаёт всё дерево общих каталогов (кроме конкретных инстансов —
    /// те создаются по одному при `instance_create`).
    pub fn ensure_shared_dirs(&self) -> std::io::Result<()> {
        for dir in [self.versions_dir(), self.libraries_dir(), self.assets_objects_dir(), self.java_dir(), self.cache_dir(), self.instances_dir()] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    /// Путь остаётся внутри `root` — защита от `slug`, полученного из
    /// пользовательского ввода (`../../..` или вложенный путь), при
    /// создании/удалении инстанса. Слаг обязан быть ровно одним сегментом
    /// пути: Windows принимает `/` наравне с `\`, поэтому проверки только
    /// на `Component::ParentDir` недостаточно — `"a/b"` тоже должен быть
    /// отклонён.
    pub fn is_instance_slug_safe(slug: &str) -> bool {
        !slug.is_empty() && !slug.contains('/') && !slug.contains('\\') && Path::new(slug).components().collect::<Vec<_>>() == vec![std::path::Component::Normal(std::ffi::OsStr::new(slug))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_expected_subpaths() {
        let paths = AppPaths::new(r"C:\Users\demo\AppData\Roaming\DreamLauncher");
        assert_eq!(paths.db_path(), PathBuf::from(r"C:\Users\demo\AppData\Roaming\DreamLauncher\launcher.db"));
        assert_eq!(paths.versions_dir(), PathBuf::from(r"C:\Users\demo\AppData\Roaming\DreamLauncher\shared\versions"));
        assert_eq!(paths.instance_game_dir("demo-pack"), PathBuf::from(r"C:\Users\demo\AppData\Roaming\DreamLauncher\instances\demo-pack\.minecraft"));
    }

    #[test]
    fn ensure_shared_dirs_creates_full_tree() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(dir.path());
        paths.ensure_shared_dirs().unwrap();

        assert!(paths.versions_dir().is_dir());
        assert!(paths.libraries_dir().is_dir());
        assert!(paths.assets_objects_dir().is_dir());
        assert!(paths.java_dir().is_dir());
        assert!(paths.instances_dir().is_dir());
    }

    #[test]
    fn rejects_unsafe_instance_slugs() {
        assert!(AppPaths::is_instance_slug_safe("survival-1.21"));
        assert!(!AppPaths::is_instance_slug_safe("../../evil"));
        assert!(!AppPaths::is_instance_slug_safe(""));
        assert!(!AppPaths::is_instance_slug_safe("a/b"));
    }
}
