//! Общий движок загрузки файлов: используется и для клиентского jar,
//! и для библиотек, и для ассетов, и для установщиков загрузчиков —
//! везде один и тот же контракт `DownloadTask -> DownloadEvent`.

use super::hash::sha1_hex;
use crate::error::{CoreError, Result};
use futures::StreamExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct DownloadTask {
    pub url: String,
    pub dest: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

#[derive(Debug, Clone)]
pub enum DownloadEvent {
    ItemSkipped { url: String },
    ItemDone { url: String },
    ItemFailed { url: String, error: String },
}

#[derive(Debug, Clone)]
pub struct DownloaderConfig {
    pub concurrency: usize,
    pub max_retries: u32,
    pub retry_base_delay: Duration,
}

impl Default for DownloaderConfig {
    fn default() -> Self {
        Self { concurrency: 16, max_retries: 3, retry_base_delay: Duration::from_millis(250) }
    }
}

/// Файл уже на диске и не требует перекачки: размер совпадает (если
/// известен), а sha1, если он есть, — тоже.
async fn already_valid(task: &DownloadTask) -> bool {
    let Ok(metadata) = tokio::fs::metadata(&task.dest).await else {
        return false;
    };
    if let Some(expected_size) = task.size {
        if metadata.len() != expected_size {
            return false;
        }
    }
    if let Some(expected_sha1) = &task.sha1 {
        match tokio::fs::read(&task.dest).await {
            Ok(bytes) => return &sha1_hex(&bytes) == expected_sha1,
            Err(_) => return false,
        }
    }
    true
}

async fn download_once(client: &reqwest::Client, task: &DownloadTask) -> Result<()> {
    let response = client.get(&task.url).send().await?.error_for_status()?;

    if let Some(parent) = task.dest.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| CoreError::io(parent.display().to_string(), e))?;
    }

    // Пишем во временный `.part`, чтобы обрыв соединения не оставил
    // битый файл под настоящим именем.
    let part_path = task.dest.with_extension(match task.dest.extension() {
        Some(ext) => format!("{}.part", ext.to_string_lossy()),
        None => "part".to_string(),
    });

    let mut file = tokio::fs::File::create(&part_path).await.map_err(|e| CoreError::io(part_path.display().to_string(), e))?;
    let mut stream = response.bytes_stream();
    let mut all_bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await.map_err(|e| CoreError::io(part_path.display().to_string(), e))?;
        if task.sha1.is_some() {
            all_bytes.extend_from_slice(&chunk);
        }
    }
    file.flush().await.map_err(|e| CoreError::io(part_path.display().to_string(), e))?;
    drop(file);

    if let Some(expected) = &task.sha1 {
        let actual = sha1_hex(&all_bytes);
        if &actual != expected {
            let _ = tokio::fs::remove_file(&part_path).await;
            return Err(CoreError::ChecksumMismatch {
                path: task.dest.display().to_string(),
                expected: expected.clone(),
                actual,
            });
        }
    }

    tokio::fs::rename(&part_path, &task.dest).await.map_err(|e| CoreError::io(task.dest.display().to_string(), e))?;
    Ok(())
}

async fn download_with_retries(client: &reqwest::Client, task: &DownloadTask, config: &DownloaderConfig) -> Result<()> {
    let mut attempt = 0;
    loop {
        match download_once(client, task).await {
            Ok(()) => return Ok(()),
            Err(err) if attempt < config.max_retries => {
                attempt += 1;
                tokio::time::sleep(config.retry_base_delay * 2u32.pow(attempt - 1)).await;
                tracing::warn!(url = %task.url, attempt, error = %err, "повтор загрузки после ошибки");
            }
            Err(err) => return Err(err),
        }
    }
}

/// Скачивает все задачи с ограниченной конкурентностью, пропуская уже
/// валидные файлы. Шлёт по одному событию на файл в `events` (получатель
/// может быть отброшен — тогда события просто перестают доставляться).
pub async fn download_all(
    client: reqwest::Client,
    tasks: Vec<DownloadTask>,
    config: DownloaderConfig,
    events: mpsc::UnboundedSender<DownloadEvent>,
) -> Result<()> {
    let client = Arc::new(client);
    let config = Arc::new(config);
    let concurrency = config.concurrency.max(1);

    let mut had_error = false;

    let mut stream = futures::stream::iter(tasks.into_iter().map(|task| {
        let client = Arc::clone(&client);
        let config = Arc::clone(&config);
        let events = events.clone();
        async move {
            if already_valid(&task).await {
                let _ = events.send(DownloadEvent::ItemSkipped { url: task.url.clone() });
                return Ok(());
            }
            match download_with_retries(&client, &task, &config).await {
                Ok(()) => {
                    let _ = events.send(DownloadEvent::ItemDone { url: task.url.clone() });
                    Ok(())
                }
                Err(err) => {
                    let _ = events.send(DownloadEvent::ItemFailed { url: task.url.clone(), error: err.to_string() });
                    Err(err)
                }
            }
        }
    }))
    .buffer_unordered(concurrency);

    while let Some(result) = stream.next().await {
        if result.is_err() {
            had_error = true;
        }
    }

    if had_error {
        Err(CoreError::Other("одна или несколько загрузок завершились ошибкой".into()))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn downloads_and_verifies_checksum() {
        let server = MockServer::start().await;
        let body = b"hello dream launcher".to_vec();
        Mock::given(method("GET"))
            .and(path("/file.jar"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body.clone()))
            .expect(1)
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("file.jar");
        let task = DownloadTask {
            url: format!("{}/file.jar", server.uri()),
            dest: dest.clone(),
            sha1: Some(sha1_hex(&body)),
            size: Some(body.len() as u64),
        };

        let (tx, mut rx) = mpsc::unbounded_channel();
        download_all(reqwest::Client::new(), vec![task], DownloaderConfig::default(), tx).await.unwrap();

        assert_eq!(tokio::fs::read(&dest).await.unwrap(), body);
        assert!(matches!(rx.recv().await, Some(DownloadEvent::ItemDone { .. })));
    }

    #[tokio::test]
    async fn skips_already_valid_file_without_network_call() {
        let server = MockServer::start().await;
        let body = b"cached content".to_vec();
        Mock::given(method("GET"))
            .and(path("/cached.jar"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body.clone()))
            .expect(0)
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("cached.jar");
        tokio::fs::write(&dest, &body).await.unwrap();

        let task = DownloadTask {
            url: format!("{}/cached.jar", server.uri()),
            dest: dest.clone(),
            sha1: Some(sha1_hex(&body)),
            size: Some(body.len() as u64),
        };

        let (tx, mut rx) = mpsc::unbounded_channel();
        download_all(reqwest::Client::new(), vec![task], DownloaderConfig::default(), tx).await.unwrap();

        assert!(matches!(rx.recv().await, Some(DownloadEvent::ItemSkipped { .. })));
    }

    #[tokio::test]
    async fn checksum_mismatch_is_reported_as_failure() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/bad.jar"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"actual content".to_vec()))
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("bad.jar");
        let task = DownloadTask {
            url: format!("{}/bad.jar", server.uri()),
            dest: dest.clone(),
            sha1: Some("0000000000000000000000000000000000000".into()),
            size: None,
        };

        let config = DownloaderConfig { max_retries: 0, ..Default::default() };

        let (tx, mut rx) = mpsc::unbounded_channel();
        let result = download_all(reqwest::Client::new(), vec![task], config, tx).await;

        assert!(result.is_err());
        assert!(matches!(rx.recv().await, Some(DownloadEvent::ItemFailed { .. })));
        assert!(!dest.exists(), "битый файл не должен появиться под настоящим именем");
    }
}
