use tauri::{
    AppHandle, Manager, Runtime,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};

mod hotkey;
use hotkey::HotkeyManager;

#[derive(Default)]
struct AppState;

fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, id: &str) {
    match id {
        "quit" => {
            app.exit(0);
        }
        _ => {}
    }
}

fn create_tray_menu<R: Runtime>(app: &AppHandle<R>) -> Menu<R> {
    let quit = MenuItem::with_id(app, "quit", "Quit", true, Some("")).unwrap();
    Menu::with_items(app, &[&quit]).unwrap()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let handle = app.handle();
            let tray_menu = create_tray_menu(&handle);
            
            // Initialize hotkey manager
            let mut hotkey_manager = HotkeyManager::new(|is_speaking| {
                println!("Speaking state changed: {}", is_speaking);
            });

            // Start the hotkey manager
            if let Err(e) = hotkey_manager.start() {
                eprintln!("Failed to start hotkey manager: {}", e);
                return Err(e.into());
            }
            
            // Store app state
            app.manage(AppState::default());
            
            let tray = TrayIconBuilder::new()
                .icon(handle.default_window_icon().unwrap().clone())
                .menu(&tray_menu)
                .on_menu_event(|app, event| {
                    handle_menu_event(app, &event.id().0);
                })
                .build(handle)?;

            app.manage(tray);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
