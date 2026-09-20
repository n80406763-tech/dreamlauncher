//! Управление Java: манифест управляемых рантаймов Mojang, выбор
//! компонента под версию, резервный источник (Adoptium). Обнаружение и
//! установка на диск — M2.

pub mod install;
pub mod runtime;
pub mod select;

pub use install::{is_installed, java_executable_path, RuntimeFilesManifest};
pub use runtime::{current_platform_key, fetch_java_runtime_manifest, JavaRuntimeManifest};
pub use select::{adoptium_latest_jre_url, required_component};
