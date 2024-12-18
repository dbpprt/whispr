mod hotkey;
mod window;
mod audio;
mod config;
mod menu;
mod setup;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_positioner::init())
        .setup(|app| {
            setup::initialize_app(app)
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
