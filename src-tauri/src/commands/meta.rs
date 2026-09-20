use crate::dto::VersionManifestDto;
use crate::error::Result;
use crate::state::AppState;

/// Манифест всех версий Minecraft. `refresh` зарезервирован под
/// серверный кэш (сейчас кэширование — на стороне фронтенда, TanStack
/// Query), поэтому пока просто игнорируется.
#[tauri::command]
#[specta::specta]
pub async fn versions_manifest(state: tauri::State<'_, AppState>, _refresh: bool) -> Result<VersionManifestDto> {
    let manifest = dream_core::meta::fetch_version_manifest(&state.http).await?;
    Ok(manifest.into())
}
