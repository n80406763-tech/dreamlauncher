//! Выбор нужной Java для версии и построение URL резервного источника
//! (Adoptium), когда управляемый рантайм Mojang недоступен для платформы
//! или версии.
//!
//! Обнаружение уже установленных JDK на диске (обход `PATH`, реестра
//! Windows, `JAVA_HOME`) и фактическая загрузка/распаковка — эта часть
//! завязана на файловую систему и переносится в M2 вместе с
//! Tauri-командой `java_detect`; здесь — только чистая логика выбора.

use crate::meta::ResolvedVersion;

/// Какой управляемый рантайм и какая мажорная версия Java нужны версии
/// игры. `ResolvedVersion::java_version` уже содержит корректное значение
/// после слияния (см. `meta::resolve::merge_chain`), включая дефолт
/// `jre-legacy`/8 для версий старше 1.17, которые не публикуют этот блок.
pub fn required_component(resolved: &ResolvedVersion) -> (&str, u32) {
    (resolved.java_version.component.as_str(), resolved.java_version.major_version)
}

/// URL резервного источника JRE (Adoptium/Eclipse Temurin), если
/// управляемый рантайм Mojang недоступен для текущей платформы.
pub fn adoptium_latest_jre_url(major_version: u32, os: &str, arch: &str) -> String {
    format!("https://api.adoptium.net/v3/assets/latest/{major_version}/hotspot?os={os}&architecture={arch}&image_type=jre&vendor=eclipse")
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
    fn modern_version_uses_declared_component() {
        let resolved = resolved_fixture("version-1.21.1.json");
        assert_eq!(required_component(&resolved), ("java-runtime-delta", 21));
    }

    #[test]
    fn legacy_version_without_java_version_defaults_to_jre8() {
        let resolved = resolved_fixture("version-1.6.4.json");
        assert_eq!(required_component(&resolved), ("jre-legacy", 8));
    }

    #[test]
    fn adoptium_url_is_well_formed() {
        let url = adoptium_latest_jre_url(21, "windows", "x64");
        assert_eq!(url, "https://api.adoptium.net/v3/assets/latest/21/hotspot?os=windows&architecture=x64&image_type=jre&vendor=eclipse");
    }
}
