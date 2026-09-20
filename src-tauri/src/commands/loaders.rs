use crate::error::{DreamError, Result};
use crate::state::AppState;
use dream_core::loaders::fabric_like::{fetch_loader_versions, FabricLikeKind};
use dream_core::loaders::forge::{fetch_forge_versions, fetch_neoforge_versions};

/// Версии загрузчика для конкретной версии Minecraft, самые новые первыми.
#[tauri::command]
#[specta::specta]
pub async fn loader_versions(
    state: tauri::State<'_, AppState>,
    loader: String,
    mc_version: String,
) -> Result<Vec<String>> {
    match loader.as_str() {
        "fabric" => Ok(
            fetch_loader_versions(&state.http, FabricLikeKind::Fabric, &mc_version)
                .await?
                .into_iter()
                .map(|v| v.version)
                .collect(),
        ),
        "quilt" => Ok(
            fetch_loader_versions(&state.http, FabricLikeKind::Quilt, &mc_version)
                .await?
                .into_iter()
                .map(|v| v.version)
                .collect(),
        ),
        "forge" => Ok(fetch_forge_versions(&state.http, &mc_version).await?),
        "neoforge" => Ok(fetch_neoforge_versions(&state.http, &mc_version).await?),
        other => Err(DreamError::Other(format!(
            "список версий загрузчика недоступен для '{other}'"
        ))),
    }
}
