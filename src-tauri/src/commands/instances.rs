use crate::dto::{CreateInstanceRequest, InstanceDto, UpdateInstanceRequest};
use crate::error::{DreamError, Result};
use crate::state::AppState;
use dream_core::storage::AppPaths;

const SUPPORTED_LOADERS: &[&str] = &["vanilla", "fabric", "quilt", "forge", "neoforge"];

fn slugify(name: &str) -> String {
    let mut slug: String = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        "instance".to_string()
    } else {
        trimmed.to_string()
    }
}

fn row_to_dto(row: &rusqlite::Row) -> rusqlite::Result<InstanceDto> {
    Ok(InstanceDto {
        id: row.get(0)?,
        slug: row.get(1)?,
        name: row.get(2)?,
        mc_version: row.get(3)?,
        loader: row.get(4)?,
        loader_version: row.get(5)?,
        min_ram_mb: row.get::<_, i64>(6)? as u32,
        max_ram_mb: row.get::<_, i64>(7)? as u32,
        extra_jvm_args: row.get(8)?,
    })
}

const INSTANCE_COLUMNS: &str =
    "id, slug, name, mc_version, loader, loader_version, min_ram_mb, max_ram_mb, extra_jvm_args";

#[tauri::command]
#[specta::specta]
pub fn instances_list(state: tauri::State<'_, AppState>) -> Result<Vec<InstanceDto>> {
    let db = state.db.lock().expect("db mutex poisoned");
    let mut stmt = db.prepare(&format!(
        "SELECT {INSTANCE_COLUMNS} FROM instances ORDER BY sort_order, created_at"
    ))?;
    let rows = stmt.query_map([], row_to_dto)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

#[tauri::command]
#[specta::specta]
pub fn instance_create(
    state: tauri::State<'_, AppState>,
    req: CreateInstanceRequest,
) -> Result<InstanceDto> {
    if !SUPPORTED_LOADERS.contains(&req.loader.as_str()) {
        return Err(DreamError::Other(format!(
            "загрузчик '{}' пока не поддерживается (доступны: {}) — Forge/NeoForge в разработке",
            req.loader,
            SUPPORTED_LOADERS.join(", ")
        )));
    }
    if req.name.trim().is_empty() {
        return Err(DreamError::Other(
            "имя инстанса не может быть пустым".into(),
        ));
    }
    if req.loader != "vanilla" && req.loader_version.as_deref().unwrap_or("").is_empty() {
        return Err(DreamError::Other(format!(
            "для загрузчика {} нужно указать его версию",
            req.loader
        )));
    }
    let loader_version = if req.loader == "vanilla" {
        None
    } else {
        req.loader_version.clone()
    };

    let id = uuid::Uuid::new_v4().to_string();
    let base_slug = slugify(&req.name);

    let db = state.db.lock().expect("db mutex poisoned");

    // Уникализируем слаг, если такой уже есть — добавляем числовой суффикс.
    let mut slug = base_slug.clone();
    let mut suffix = 2;
    loop {
        let exists: i64 = db.query_row(
            "SELECT COUNT(*) FROM instances WHERE slug = ?1",
            [&slug],
            |r| r.get(0),
        )?;
        if exists == 0 {
            break;
        }
        slug = format!("{base_slug}-{suffix}");
        suffix += 1;
    }
    if !AppPaths::is_instance_slug_safe(&slug) {
        return Err(DreamError::Other(format!(
            "не удалось построить безопасное имя каталога из '{}': {}",
            req.name, slug
        )));
    }

    db.execute(
        "INSERT INTO instances (id, slug, name, mc_version, loader, loader_version) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![id, slug, req.name, req.mc_version, req.loader, loader_version],
    )?;

    let game_dir = state.paths.instance_game_dir(&slug);
    std::fs::create_dir_all(&game_dir)?;

    // --- Inject Space Theme Resource Pack ---
    let rp_dir = game_dir.join("resourcepacks");
    std::fs::create_dir_all(&rp_dir).ok();

    // Embed the space theme resource pack directly in the binary
    let space_rp_path = rp_dir.join("space_gui.zip");
    let zip_data = include_bytes!("../../assets/space_gui.zip");
    std::fs::write(&space_rp_path, zip_data).ok();

    // Enable it in options.txt
    let options_txt_path = game_dir.join("options.txt");
    let options_content = "resourcePacks:[\"vanilla\",\"file/space_gui.zip\"]\n";
    std::fs::write(&options_txt_path, options_content).ok();
    // -----------------------------------------

    Ok(InstanceDto {
        id,
        slug,
        name: req.name,
        mc_version: req.mc_version,
        loader: req.loader,
        loader_version,
        min_ram_mb: 512,
        max_ram_mb: 4096,
        extra_jvm_args: String::new(),
    })
}

/// RAM и дополнительные JVM-аргументы — версия игры/загрузчик не
/// редактируются намеренно (см. `UpdateInstanceRequest`).
#[tauri::command]
#[specta::specta]
pub fn instance_update(
    state: tauri::State<'_, AppState>,
    req: UpdateInstanceRequest,
) -> Result<InstanceDto> {
    if req.min_ram_mb == 0 || req.min_ram_mb > req.max_ram_mb {
        return Err(DreamError::Other(
            "минимум RAM должен быть больше 0 и не больше максимума".into(),
        ));
    }
    let db = state.db.lock().expect("db mutex poisoned");
    db.execute(
        "UPDATE instances SET min_ram_mb = ?1, max_ram_mb = ?2, extra_jvm_args = ?3 WHERE id = ?4",
        rusqlite::params![req.min_ram_mb, req.max_ram_mb, req.extra_jvm_args, req.id],
    )?;
    db.query_row(
        &format!("SELECT {INSTANCE_COLUMNS} FROM instances WHERE id = ?1"),
        [&req.id],
        row_to_dto,
    )
    .map_err(|_| DreamError::Other(format!("инстанс {} не найден", req.id)))
}

/// Открывает папку игры (`.minecraft`) инстанса в проводнике.
#[tauri::command]
#[specta::specta]
pub fn instance_open_folder(state: tauri::State<'_, AppState>, id: String) -> Result<()> {
    let db = state.db.lock().expect("db mutex poisoned");
    let slug: String = db
        .query_row("SELECT slug FROM instances WHERE id = ?1", [&id], |r| {
            r.get(0)
        })
        .map_err(|_| DreamError::Other(format!("инстанс {id} не найден")))?;
    drop(db);
    let dir = state.paths.instance_game_dir(&slug);
    std::fs::create_dir_all(&dir)?;
    tauri_plugin_opener::open_path(dir, None::<&str>)
        .map_err(|e| DreamError::Other(format!("не удалось открыть папку: {e}")))
}

#[tauri::command]
#[specta::specta]
pub fn instance_delete(state: tauri::State<'_, AppState>, id: String) -> Result<()> {
    let db = state.db.lock().expect("db mutex poisoned");
    let slug: Option<String> = db
        .query_row("SELECT slug FROM instances WHERE id = ?1", [&id], |r| {
            r.get(0)
        })
        .ok();
    db.execute("DELETE FROM instances WHERE id = ?1", [&id])?;
    drop(db);

    if let Some(slug) = slug {
        let dir = state.paths.instance_dir(&slug);
        if dir.exists() {
            std::fs::remove_dir_all(dir)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_normalizes_names() {
        assert_eq!(slugify("My Cool Pack!"), "my-cool-pack");
        assert_eq!(slugify("  spaced  "), "spaced");
        assert_eq!(slugify("Кириллица"), "instance");
        assert_eq!(slugify(""), "instance");
    }
}
