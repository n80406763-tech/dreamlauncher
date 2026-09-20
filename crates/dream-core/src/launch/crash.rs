//! Детект и обработка крэш-репортов Minecraft.
//! Ищет свежие `hs_err_pid*.log` (JVM-крэши) и `crash-reports/*.txt`
//! (игровые крэши) в папке инстанса и предоставляет их содержимое для UI.

use crate::error::Result;
use std::path::{Path, PathBuf};

/// Крэш-репорт: путь к файлу + фрагмент содержимого для превью.
#[derive(Debug, Clone)]
pub struct CrashReport {
    pub path: PathBuf,
    pub kind: CrashKind,
    pub preview: String,
    pub modified: std::time::SystemTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashKind {
    /// JVM fatal error (SIGSEGV, OutOfMemory и т.п.)
    JvmFatal,
    /// Игровой крэш (исключение в Minecraft/модах)
    GameCrash,
}

/// Ищет свежие крэш-репорты в папке инстанса. Возвращает до `limit`
/// репортов, отсортированных по времени модификации (свежие первыми).
pub fn find_recent_crashes(game_dir: &Path, limit: usize) -> Result<Vec<CrashReport>> {
    let mut reports = Vec::new();

    // JVM-крэши: hs_err_pid*.log в корне gameDir
    if let Ok(entries) = std::fs::read_dir(game_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("hs_err_pid") && name.ends_with(".log") {
                    if let Ok(meta) = entry.metadata() {
                        if let Ok(modified) = meta.modified() {
                            if let Ok(preview) = read_preview(&path, 10) {
                                reports.push(CrashReport { path, kind: CrashKind::JvmFatal, preview, modified });
                            }
                        }
                    }
                }
            }
        }
    }

    // Игровые крэши: crash-reports/*.txt
    let crash_reports_dir = game_dir.join("crash-reports");
    if crash_reports_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&crash_reports_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("txt") {
                    if let Ok(meta) = entry.metadata() {
                        if let Ok(modified) = meta.modified() {
                            if let Ok(preview) = read_preview(&path, 10) {
                                reports.push(CrashReport { path, kind: CrashKind::GameCrash, preview, modified });
                            }
                        }
                    }
                }
            }
        }
    }

    // Сортируем по времени (свежие первыми), ограничиваем
    reports.sort_by(|a, b| b.modified.cmp(&a.modified));
    reports.truncate(limit);
    Ok(reports)
}

/// Читает полное содержимое крэш-репорта. Ограничено 1 МБ для безопасности.
pub fn read_crash_report(path: &Path) -> Result<String> {
    const MAX_SIZE: u64 = 1024 * 1024; // 1 MB
    let meta = std::fs::metadata(path).map_err(|e| crate::error::CoreError::io(path.display().to_string(), e))?;
    if meta.len() > MAX_SIZE {
        return Err(crate::error::CoreError::Other(format!("крэш-репорт слишком большой: {} байт (лимит {} байт)", meta.len(), MAX_SIZE)));
    }
    let content = std::fs::read_to_string(path).map_err(|e| crate::error::CoreError::io(path.display().to_string(), e))?;
    Ok(content)
}

/// Читает первые `lines` строк файла для превью.
fn read_preview(path: &Path, lines: usize) -> Result<String> {
    let content = std::fs::read_to_string(path).map_err(|e| crate::error::CoreError::io(path.display().to_string(), e))?;
    let preview: String = content.lines().take(lines).collect::<Vec<_>>().join("\n");
    Ok(preview)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn finds_jvm_crash_in_game_dir() {
        let dir = tempfile::tempdir().unwrap();
        let crash_path = dir.path().join("hs_err_pid12345.log");
        let mut f = std::fs::File::create(&crash_path).unwrap();
        writeln!(f, "# JVM fatal error").unwrap();
        writeln!(f, "SIGSEGV (0xb) at pc=0x00007fff").unwrap();

        let reports = find_recent_crashes(dir.path(), 10).unwrap();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].kind, CrashKind::JvmFatal);
        assert!(reports[0].preview.contains("JVM fatal error"));
    }

    #[test]
    fn finds_game_crash_in_crash_reports_subdir() {
        let dir = tempfile::tempdir().unwrap();
        let crash_dir = dir.path().join("crash-reports");
        std::fs::create_dir(&crash_dir).unwrap();
        let crash_path = crash_dir.join("crash-2026-09-07_12-34-56-client.txt");
        let mut f = std::fs::File::create(&crash_path).unwrap();
        writeln!(f, "---- Minecraft Crash Report ----").unwrap();
        writeln!(f, "// Ouch. That hurt :(").unwrap();

        let reports = find_recent_crashes(dir.path(), 10).unwrap();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].kind, CrashKind::GameCrash);
        assert!(reports[0].preview.contains("Minecraft Crash Report"));
    }

    #[test]
    fn reads_full_crash_report() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("crash.txt");
        std::fs::write(&path, "Full crash content\nLine 2\nLine 3").unwrap();

        let content = read_crash_report(&path).unwrap();
        assert_eq!(content, "Full crash content\nLine 2\nLine 3");
    }

    #[test]
    fn limits_number_of_returned_crashes() {
        let dir = tempfile::tempdir().unwrap();
        let crash_dir = dir.path().join("crash-reports");
        std::fs::create_dir(&crash_dir).unwrap();

        for i in 0..5 {
            let path = crash_dir.join(format!("crash-{}.txt", i));
            std::fs::write(&path, format!("Crash {}", i)).unwrap();
        }

        let reports = find_recent_crashes(dir.path(), 3).unwrap();
        assert_eq!(reports.len(), 3);
    }
}
