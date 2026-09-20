//! Чтение содержимого installer-jar Forge/NeoForge: `install_profile.json`,
//! `version.json`, произвольные файлы из `data/` и `Main-Class` из
//! манифеста jar-инструментов процессоров — всё через `zip`, без
//! распаковки на диск (installer нужен только на время установки).

use crate::error::{CoreError, Result};
use std::io::Read;

/// Открывает архив в памяти и возвращает текст одной записи.
pub fn read_zip_entry_text(bytes: &[u8], entry_name: &str) -> Result<String> {
    let bytes_vec = read_zip_entry_bytes(bytes, entry_name)?;
    String::from_utf8(bytes_vec).map_err(|e| CoreError::Other(format!("запись {entry_name} в архиве — не валидный UTF-8: {e}")))
}

pub fn read_zip_entry_bytes(bytes: &[u8], entry_name: &str) -> Result<Vec<u8>> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| CoreError::Other(format!("архив повреждён: {e}")))?;
    let mut entry = archive.by_name(entry_name).map_err(|_| CoreError::Other(format!("в архиве нет записи {entry_name}")))?;
    let mut buf = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut buf).map_err(|e| CoreError::io(entry_name.to_string(), e))?;
    Ok(buf)
}

/// Значение `Main-Class` из `META-INF/MANIFEST.MF` jar-а — вручную, без
/// полноценного парсера манифестов: формат простой (`Ключ: значение` по
/// строке, с возможным переносом длинных строк на следующую с ведущим
/// пробелом — переносы нам тут не мешают, `Main-Class` короткий).
pub fn read_main_class(jar_bytes: &[u8]) -> Result<String> {
    let manifest = read_zip_entry_text(jar_bytes, "META-INF/MANIFEST.MF")?;
    for line in manifest.lines() {
        if let Some(value) = line.strip_prefix("Main-Class:") {
            return Ok(value.trim().to_string());
        }
    }
    Err(CoreError::Other("в манифесте jar нет Main-Class".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn build_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut cursor);
            let options = SimpleFileOptions::default();
            for (name, content) in entries {
                writer.start_file(*name, options).unwrap();
                writer.write_all(content).unwrap();
            }
            writer.finish().unwrap();
        }
        cursor.into_inner()
    }

    #[test]
    fn reads_text_entry() {
        let zip = build_zip(&[("install_profile.json", b"{\"a\":1}")]);
        assert_eq!(read_zip_entry_text(&zip, "install_profile.json").unwrap(), "{\"a\":1}");
        assert!(read_zip_entry_text(&zip, "missing.json").is_err());
    }

    #[test]
    fn extracts_main_class_from_manifest() {
        let manifest = b"Manifest-Version: 1.0\r\nMain-Class: net.minecraftforge.installertools.ConsoleTool\r\nImplementation-Vendor: Forge\r\n";
        let zip = build_zip(&[("META-INF/MANIFEST.MF", manifest)]);
        assert_eq!(read_main_class(&zip).unwrap(), "net.minecraftforge.installertools.ConsoleTool");
    }

    #[test]
    fn reads_real_installer_fixture_entries() {
        // forge-1.20.1-install_profile.json — тот же файл, что уже
        // используется тестами InstallProfile; здесь просто проверяем,
        // что чтение "сырого" текста через zip-путь тоже сработало бы на
        // реальных данных (используем тот же файл напрямую как текст,
        // не как zip — тест архива целиком с реальным installer.jar не
        // хранится в фикстурах из-за размера, см. install::tests
        // (`#[ignore]`) для полноценной сетевой проверки).
        let path = format!("{}/tests/fixtures/forge-1.20.1-install_profile.json", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(path).unwrap();
        assert!(text.contains("\"spec\""));
    }
}
