pub mod accounts;
pub mod crashes;
pub mod install;
pub mod instances;
pub mod launch;
pub mod loaders;
pub mod meta;
pub mod mods;
pub mod shaders;

pub use accounts::{account_add_microsoft, account_add_offline, account_refresh_microsoft, account_remove, account_set_active, accounts_list};
pub use crashes::{crash_read, crashes_list};
pub use install::install_version;
pub use instances::{instance_create, instance_delete, instance_open_folder, instance_update, instances_list};
pub use launch::launch_instance;
pub use loaders::loader_versions;
pub use meta::versions_manifest;
pub use mods::{mods_install, mods_list, mods_remove, mods_search, mods_toggle};
pub use shaders::{shaders_install, shaders_list, shaders_remove, shaders_search, shaders_set_active};

use crate::error::{DreamError, Result};
use dream_core::loaders::LoaderKind;

pub(crate) fn parse_loader(loader: &str) -> Result<LoaderKind> {
    match loader {
        "vanilla" => Ok(LoaderKind::Vanilla),
        "fabric" => Ok(LoaderKind::Fabric),
        "quilt" => Ok(LoaderKind::Quilt),
        "forge" => Ok(LoaderKind::Forge),
        "neoforge" => Ok(LoaderKind::NeoForge),
        other => Err(DreamError::Other(format!("неизвестный загрузчик: {other}"))),
    }
}
