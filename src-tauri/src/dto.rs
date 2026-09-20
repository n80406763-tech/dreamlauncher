//! DTO-типы для IPC. Отдельно от `dream_core`, чтобы ядро оставалось
//! свободным от `specta`/Tauri, а фронтенду доставались только те поля,
//! которые ему реально нужны.

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Type)]
pub struct LatestVersionsDto {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct VersionEntryDto {
    pub id: String,
    pub kind: String,
    pub release_time: String,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct VersionManifestDto {
    pub latest: LatestVersionsDto,
    pub versions: Vec<VersionEntryDto>,
}

impl From<dream_core::meta::VersionManifest> for VersionManifestDto {
    fn from(m: dream_core::meta::VersionManifest) -> Self {
        Self {
            latest: LatestVersionsDto {
                release: m.latest.release,
                snapshot: m.latest.snapshot,
            },
            versions: m
                .versions
                .into_iter()
                .map(|v| VersionEntryDto {
                    id: v.id,
                    kind: format!("{:?}", v.kind).to_lowercase(),
                    release_time: v.release_time,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct AccountDto {
    pub id: String,
    pub username: String,
    pub kind: String, // "microsoft" | "offline"
    pub is_active: bool,
}

#[derive(Debug, Clone, Deserialize, Type)]
pub struct CreateInstanceRequest {
    pub name: String,
    pub mc_version: String,
    pub loader: String, // "vanilla" | "fabric" | "quilt" | "forge" | "neoforge"
    /// Обязателен для fabric/quilt, игнорируется для vanilla.
    pub loader_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct InstanceDto {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub mc_version: String,
    pub loader: String,
    pub loader_version: Option<String>,
    pub min_ram_mb: u32,
    pub max_ram_mb: u32,
    pub extra_jvm_args: String,
}

/// Изменяемые поля инстанса — RAM и дополнительные JVM-аргументы. Версия
/// игры/загрузчик намеренно не редактируются (это фактически новый
/// инстанс — проще пересоздать, чем мигрировать установленные файлы).
#[derive(Debug, Clone, Deserialize, Type)]
pub struct UpdateInstanceRequest {
    pub id: String,
    pub min_ram_mb: u32,
    pub max_ram_mb: u32,
    pub extra_jvm_args: String,
}

/// Прогресс установки версии — один канал на весь процесс: скачивание
/// метаданных, файлов, распаковка натив-либ и (для старых версий)
/// раскладка ассетов.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "type")]
pub enum InstallProgressEvent {
    Phase {
        message: String,
    },
    /// `done`/`total` считаются по файлам (не по байтам) — событие
    /// приходит на каждый обработанный файл.
    Progress {
        done: u32,
        total: u32,
    },
    ItemFailed {
        url: String,
        error: String,
    },
    Done,
    Error {
        message: String,
    },
}

/// События запущенного процесса игры.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "type")]
pub enum LaunchEventDto {
    Starting,
    Stdout { line: String },
    Stderr { line: String },
    Exited { code: Option<i32> },
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct ModSearchHitDto {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    // u32, не u64/u32-as-is: specta отказывается экспортировать 64-битные
    // числа в TS (риск потери точности в BigInt) — счётчик загрузок мода
    // никогда не подойдёт к границе u32.
    pub downloads: u32,
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct ModSearchResultDto {
    pub hits: Vec<ModSearchHitDto>,
    pub total_hits: u32,
}

impl From<dream_core::mods::modrinth::SearchHit> for ModSearchHitDto {
    fn from(h: dream_core::mods::modrinth::SearchHit) -> Self {
        Self {
            project_id: h.project_id,
            slug: h.slug,
            title: h.title,
            description: h.description,
            author: h.author,
            downloads: h.downloads.min(u32::MAX as u64) as u32,
            icon_url: h.icon_url,
        }
    }
}

impl From<dream_core::mods::SearchResponse> for ModSearchResultDto {
    fn from(r: dream_core::mods::SearchResponse) -> Self {
        Self {
            hits: r.hits.into_iter().map(Into::into).collect(),
            total_hits: r.total_hits.min(u32::MAX as u64) as u32,
        }
    }
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct InstalledModDto {
    pub project_id: String,
    pub version_id: String,
    pub filename: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct InstalledShaderDto {
    pub project_id: String,
    pub version_id: String,
    pub filename: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct ShaderSearchResultDto {
    pub hits: Vec<ModSearchHitDto>,
    pub total_hits: u32,
}

impl From<dream_core::mods::SearchResponse> for ShaderSearchResultDto {
    fn from(r: dream_core::mods::SearchResponse) -> Self {
        Self {
            hits: r.hits.into_iter().map(Into::into).collect(),
            total_hits: r.total_hits.min(u32::MAX as u64) as u32,
        }
    }
}

/// Прогресс Microsoft OAuth авторизации — отправляется через Channel.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "type")]
pub enum AuthProgressDto {
    WaitingForUser {
        user_code: String,
        verification_uri: String,
        expires_in: u32,
    },
    Polling,
    ExchangingXboxLive,
    ExchangingXsts,
    LoggingIntoMinecraft,
    FetchingProfile,
    Complete {
        uuid: String,
        username: String,
    },
    Error {
        message: String,
    },
}

/// Крэш-репорт Minecraft.
#[derive(Debug, Clone, Serialize, Type)]
pub struct CrashReportDto {
    pub path: String,
    pub kind: String, // "jvm_fatal" | "game_crash"
    pub preview: String,
    pub modified: u32, // Секунды с последней модификации (относительно now)
}
