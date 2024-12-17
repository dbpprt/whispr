use tauri::{
    AppHandle, Manager, Runtime,
    menu::{Menu, MenuItem, Submenu, CheckMenuItem},
    tray::TrayIconBuilder,
};
mod hotkey;
mod window;
mod audio;

use hotkey::HotkeyManager;
use window::OverlayWindow;
use audio::AudioManager;
use std::sync::Mutex;

fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, id: &str) {
    match id {
        "quit" => {
            app.exit(0);
        }
        _ => {}
    }
}

fn create_tray_menu<R: Runtime>(app: &AppHandle<R>) -> Menu<R> {
    // Create quit menu item
    let quit = MenuItem::new(app, "Quit".to_string(), true, None::<String>).unwrap();

    // Create audio device menu items
    let mut audio_device_items = Vec::new();
    if let Ok(audio_manager) = AudioManager::new() {
        if let Ok(devices) = audio_manager.list_input_devices() {
            if let Ok(active_device_name) = audio_manager.get_current_device_name() {
                for device in devices {
                    let is_active = device == active_device_name;
                    let item = CheckMenuItem::with_id(app, &device, &device, true, is_active, None::<String>).unwrap();
                    audio_device_items.push(item);
                }
            } else {
                eprintln!("Failed to get current device name");
            }
        }
    }

    // Convert audio device items to IsMenuItem trait objects
    let audio_device_refs: Vec<&dyn tauri::menu::IsMenuItem<R>> = audio_device_items
        .iter()
        .map(|item| item as &dyn tauri::menu::IsMenuItem<R>)
        .collect();

    // Create the audio device submenu
    let audio_submenu = Submenu::with_items(
        app,
        "Audio Device",
        true,
        &audio_device_refs
    ).unwrap();

    // Create the main menu with all items
    let main_items: Vec<&dyn tauri::menu::IsMenuItem<R>> = vec![
        &quit,
        &audio_submenu
    ];
    Menu::with_items(app, &main_items).unwrap()
}

fn handle_audio_device_selection<R: Runtime>(app: &AppHandle<R>, id: &str) {
    if let Some(audio_state) = app.try_state::<Mutex<AudioManager>>() {
        let mut audio_manager = audio_state.lock().unwrap();
        if let Err(e) = audio_manager.set_input_device(id) {
            eprintln!("Failed to set input device: {}", e);
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let handle = app.handle();
            let tray_menu = create_tray_menu(&handle);
            
            // Create and store the overlay window
            let mut overlay_window = OverlayWindow::new();
            overlay_window.create_window(&handle);
            let overlay_window = Mutex::new(overlay_window);
            app.manage(overlay_window);
            
            // Initialize audio manager
            let audio_manager = AudioManager::new().expect("Failed to initialize audio manager");
            let audio_manager = Mutex::new(audio_manager);
            app.manage(audio_manager);
            
            // Initialize hotkey manager with window and audio control
            let app_handle = handle.clone();
            let mut hotkey_manager = HotkeyManager::new(move |is_speaking| {
                println!("Speaking state changed: {}", is_speaking);
                
                // Control window visibility
                if let Some(window_state) = app_handle.try_state::<Mutex<OverlayWindow>>() {
                    let window = window_state.lock().unwrap();
                    if is_speaking {
                        window.show();
                    } else {
                        window.hide();
                    }
                }

                // Control audio capture
                if let Some(audio_state) = app_handle.try_state::<Mutex<AudioManager>>() {
                    let mut audio = audio_state.lock().unwrap();
                    if is_speaking {
                        if let Err(e) = audio.start_capture() {
                            eprintln!("Failed to start audio capture: {}", e);
                        }
                    } else {
                        audio.stop_capture();
                    }
                }
            });

            // Start the hotkey manager
            if let Err(e) = hotkey_manager.start() {
                eprintln!("Failed to start hotkey manager: {}", e);
                return Err(e.into());
            }
            
            // Create system tray
            let tray = TrayIconBuilder::new()
                .icon(handle.default_window_icon().unwrap().clone())
                .menu(&tray_menu)
                .on_menu_event(|app, event| {
                    handle_audio_device_selection(app, &event.id().0);
                })
                .build(app)?;
            
            app.manage(tray);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
