//! Сборка classpath из разрешённых (по `rules`) библиотек версии.

use crate::meta::{ResolvedVersion, RuleContext};
use std::path::{Path, PathBuf};

/// Абсолютные пути ко всем библиотечным jar-файлам, которые нужно положить
/// в classpath, в порядке из `ResolvedVersion` (загрузчик впереди ванили).
pub fn library_paths(resolved: &ResolvedVersion, libraries_dir: &Path, ctx: &RuleContext) -> Vec<PathBuf> {
    resolved
        .applicable_libraries(ctx)
        .into_iter()
        .filter_map(|lib| lib.main_artifact())
        .map(|artifact| libraries_dir.join(artifact.relative_path.replace('/', std::path::MAIN_SEPARATOR_STR)))
        .collect()
}

/// Полная classpath-строка. `include_client_jar` выключают для Forge
/// ≥1.17, где клиентский jar подключается модульной системой ModLauncher,
/// а не через `-cp`.
pub fn build_classpath(
    resolved: &ResolvedVersion,
    libraries_dir: &Path,
    client_jar: &Path,
    include_client_jar: bool,
    separator: &str,
) -> String {
    let mut paths = library_paths(resolved, libraries_dir, &RuleContext::current());
    if include_client_jar {
        paths.push(client_jar.to_path_buf());
    }
    paths.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>().join(separator)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::merge_chain;

    fn resolved_fixture(name: &str) -> ResolvedVersion {
        let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(path).unwrap();
        let raw = serde_json::from_str(&text).unwrap();
        merge_chain(vec![raw]).unwrap()
    }

    #[test]
    fn classpath_includes_client_jar_when_requested() {
        let resolved = resolved_fixture("version-1.12.2.json");
        let libs_dir = PathBuf::from(r"C:\shared\libraries");
        let client = PathBuf::from(r"C:\shared\versions\1.12.2\1.12.2.jar");

        let with_client = build_classpath(&resolved, &libs_dir, &client, true, ";");
        let without_client = build_classpath(&resolved, &libs_dir, &client, false, ";");

        assert!(with_client.ends_with("1.12.2.jar"));
        assert!(!without_client.contains("1.12.2.jar"));
        assert!(with_client.len() > without_client.len());
    }

    #[test]
    fn excludes_libraries_not_allowed_on_current_os() {
        let resolved = resolved_fixture("version-1.21.1.json");
        let ctx = RuleContext { os_name: "windows", os_arch: "x86_64", features: Default::default() };
        let paths = library_paths(&resolved, &PathBuf::from(r"C:\shared\libraries"), &ctx);
        // java-objc-bridge разрешён только на osx — не должен попасть в classpath на Windows.
        assert!(!paths.iter().any(|p| p.to_string_lossy().contains("java-objc-bridge")));
        // Обычная кросс-платформенная библиотека должна присутствовать.
        assert!(paths.iter().any(|p| p.to_string_lossy().contains("gson")));
    }
}
