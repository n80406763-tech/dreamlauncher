//! Безопасная распаковка zip-архивов: natives-джары, `.mrpack`, модпаки
//! CurseForge и установщики Forge/NeoForge — всё проходит через эту функцию.
//!
//! Защита от zip-slip в два слоя: полагаемся на `ZipFile::enclosed_name()`
//! (отклоняет `..` и абсолютные пути), а затем ещё раз проверяем, что
//! итоговый путь остаётся внутри каталога назначения — на случай багов в
//! обработке путей на конкретной ОС.

use crate::error::{CoreError, Result};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

/// Список префиксов, которые не нужно извлекать (метаданные подписи jar
/// и т.п.) — используется при распаковке natives.
#[derive(Default)]
pub struct ExtractOptions<'a> {
    pub exclude_prefixes: &'a [String],
}

/// `relative` уже прошёл через `enclosed_name()`, но мы всё равно
/// перепроверяем сами: он не должен содержать `..`/абсолютных
/// компонентов, а итоговый путь — обязан остаться внутри `root`.
/// (Важно проверять компоненты именно `relative`, а не `candidate` —
/// у `candidate = root.join(relative)` компонент `Prefix` есть всегда,
/// он приходит из `root`, и это нормально.)
fn is_within(root: &Path, relative: &Path, candidate: &Path) -> bool {
    relative.components().all(|c| matches!(c, Component::Normal(_))) && candidate.starts_with(root)
}

/// Распаковывает архив из `bytes` в `dest_dir`, создавая его при
/// необходимости. Возвращает список извлечённых файлов (абсолютные пути).
pub fn safe_extract(bytes: &[u8], dest_dir: &Path, opts: &ExtractOptions) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dest_dir).map_err(|e| CoreError::io(dest_dir.display().to_string(), e))?;
    let dest_dir = dunce::canonicalize(dest_dir).map_err(|e| CoreError::io(dest_dir.display().to_string(), e))?;

    let reader = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| CoreError::Other(format!("архив повреждён: {e}")))?;

    let mut extracted = Vec::new();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| CoreError::Other(format!("не удалось прочитать запись {i}: {e}")))?;

        // `enclosed_name()` возвращает None для путей с `..`, абсолютных
        // путей и путей с диском на Windows — ровно то, от чего защищаемся.
        let Some(relative) = entry.enclosed_name() else {
            return Err(CoreError::UnsafeArchivePath(entry.name().to_string()));
        };

        // Записи в zip всегда используют `/` как разделитель, а
        // `enclosed_name()` возвращает `PathBuf`, который на Windows
        // печатается через `\` — нормализуем перед сравнением с
        // префиксами вида "META-INF/".
        let relative_str = relative.to_string_lossy().replace('\\', "/");
        if opts.exclude_prefixes.iter().any(|p| relative_str.starts_with(p.as_str())) {
            continue;
        }

        let out_path = dest_dir.join(&relative);

        // Второй, независимый от библиотеки барьер: итоговый путь обязан
        // остаться внутри dest_dir.
        if !is_within(&dest_dir, &relative, &out_path) {
            return Err(CoreError::UnsafeArchivePath(entry.name().to_string()));
        }

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| CoreError::io(out_path.display().to_string(), e))?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| CoreError::io(parent.display().to_string(), e))?;
        }

        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf).map_err(|e| CoreError::io(out_path.display().to_string(), e))?;
        std::fs::write(&out_path, &buf).map_err(|e| CoreError::io(out_path.display().to_string(), e))?;

        extracted.push(out_path);
    }

    Ok(extracted)
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
    fn extracts_well_formed_archive() {
        let dir = tempfile::tempdir().unwrap();
        let zip_bytes = build_zip(&[("hello.txt", b"hi"), ("nested/deep.txt", b"deep")]);

        let extracted = safe_extract(&zip_bytes, dir.path(), &ExtractOptions::default()).unwrap();

        assert_eq!(extracted.len(), 2);
        assert_eq!(std::fs::read_to_string(dir.path().join("hello.txt")).unwrap(), "hi");
        assert_eq!(std::fs::read_to_string(dir.path().join("nested/deep.txt")).unwrap(), "deep");
    }

    #[test]
    fn rejects_zip_slip_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let zip_bytes = build_zip(&[("../../evil.txt", b"pwned")]);

        let result = safe_extract(&zip_bytes, dir.path(), &ExtractOptions::default());

        assert!(matches!(result, Err(CoreError::UnsafeArchivePath(_))));
        // За пределами каталога назначения ничего не должно появиться.
        assert!(!dir.path().parent().unwrap().join("evil.txt").exists());
    }

    #[test]
    fn excludes_configured_prefixes() {
        let dir = tempfile::tempdir().unwrap();
        let zip_bytes = build_zip(&[("META-INF/MANIFEST.MF", b"x"), ("lib.dll", b"native")]);

        let extracted = safe_extract(&zip_bytes, dir.path(), &ExtractOptions { exclude_prefixes: &["META-INF/".to_string()] }).unwrap();

        assert_eq!(extracted.len(), 1);
        assert!(!dir.path().join("META-INF/MANIFEST.MF").exists());
        assert!(dir.path().join("lib.dll").exists());
    }
}
