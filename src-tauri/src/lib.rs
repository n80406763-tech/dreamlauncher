pub mod commands;
pub mod dto;
pub mod error;
pub mod state;

use state::AppState;
use tauri::Manager;
use tauri_specta::{collect_commands, Builder};

fn build_specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        commands::meta::versions_manifest,
        commands::accounts::accounts_list,
        commands::accounts::account_add_offline,
        commands::accounts::account_add_microsoft,
        commands::accounts::account_refresh_microsoft,
        commands::accounts::account_set_active,
        commands::accounts::account_remove,
        commands::instances::instances_list,
        commands::instances::instance_create,
        commands::instances::instance_update,
        commands::instances::instance_open_folder,
        commands::instances::instance_delete,
        commands::loaders::loader_versions,
        commands::install::install_version,
        commands::launch::launch_instance,
        commands::mods::mods_search,
        commands::mods::mods_list,
        commands::mods::mods_install,
        commands::mods::mods_toggle,
        commands::mods::mods_remove,
        commands::shaders::shaders_search,
        commands::shaders::shaders_list,
        commands::shaders::shaders_install,
        commands::shaders::shaders_set_active,
        commands::shaders::shaders_remove,
        commands::crashes::crashes_list,
        commands::crashes::crash_read,
    ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let specta_builder = build_specta_builder();

    #[cfg(debug_assertions)]
    specta_builder
        .export(specta_typescript::Typescript::default(), "../src/ipc/bindings.ts")
        .expect("не удалось экспортировать TypeScript-биндинги для IPC");

    // Однократный неинтерактивный прогон только ради экспорта биндингов —
    // используется `npm run gen:bindings`, окно не открывается.
    if std::env::var_os("DREAMLAUNCHER_EXPORT_BINDINGS_ONLY").is_some() {
        return;
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // Кто-то запустил второй экземпляр — просто поднимаем существующее окно.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(tauri_plugin_log::Builder::default().level(log::LevelFilter::Info).build())?;
            }

            let state = AppState::init().map_err(|e| -> Box<dyn std::error::Error> { Box::new(std::io::Error::other(e.to_string())) })?;
            app.manage(state);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
