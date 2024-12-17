use tauri::{
    AppHandle, Manager, Runtime,
    menu::{Menu, MenuItem, Submenu, CheckMenuItem},
    tray::TrayIconBuilder,
    State,
};
mod hotkey;
mod window;
mod audio;

use hotkey::HotkeyManager;
use window::OverlayWindow;
use audio::AudioManager;
use std::sync::Mutex;
use std::collections::HashMap;

// State struct to hold menu items
#[derive(Default)]
struct MenuState<R: Runtime> {
    audio_device_map: Mutex<HashMap<String, CheckMenuItem<R>>>,
}

fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, id: &str) {
    match id {
        "quit" => {
            app.exit(0);
        }
        _ => {}
    }
}

fn create_tray_menu<R: Runtime>(app: &AppHandle<R>) -> (Menu<R>, HashMap<String, CheckMenuItem<R>>) {
    // Create quit menu item
    let quit = MenuItem::new(app, "Quit".to_string(), true, None::<String>).unwrap();

    // Create audio device menu items
    let mut audio_device_items = Vec::new();
    let mut audio_device_map = HashMap::new();
    if let Ok(audio_manager) = AudioManager::new() {
        if let Ok(devices) = audio_manager.list_input_devices() {
            if let Ok(active_device_name) = audio_manager.get_current_device_name() {
                for device in devices {
                    let is_active = device == active_device_name;
                    let item = CheckMenuItem::with_id(app, &device, &device, true, is_active, None::<String>).unwrap();
                    audio_device_items.push(item.clone());
                    audio_device_map.insert(device.to_string(), item);
                }
            } else {
                eprintln!("Failed to get current device name");
            }
        }
    }

    // Convert audio device items to IsMenuItem trait objects
    let audio_device_refs: Vec<&dyn tauri::menu::IsMenuItem<R>> = audio_device_items.iter()
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
    (Menu::with_items(app, &main_items).unwrap(), audio_device_map)
}

fn handle_audio_device_selection<R: Runtime>(app: AppHandle<R>, id: &str, audio_device_map: &HashMap<String, CheckMenuItem<R>>) {
    if let Some(audio_state) = app.try_state::<Mutex<AudioManager>>() {
        let mut audio_manager = audio_state.lock().unwrap();
        if let Err(e) = audio_manager.set_input_device(id) {
            eprintln!("Failed to set input device: {}", e);
            // Ensure menu state remains consistent with actual device state
            if let Ok(current_device) = audio_manager.get_current_device_name() {
                // Reset all checkmarks
                for (device_id, item) in audio_device_map {
                    item.set_checked(device_id == &current_device).unwrap();
                }
            }
        } else {
            // Successfully changed device - update menu state
            for (device_id, item) in audio_device_map {
                item.set_checked(device_id == id).unwrap();
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let handle = app.handle();
            let (tray_menu, audio_device_map) = create_tray_menu(&handle);
            
            // Store menu state
            app.manage(MenuState { 
                audio_device_map: Mutex::new(audio_device_map)
            });
            
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
            
            // Create system tray with audio device menu
            let handle_clone = handle.clone();
            let tray = TrayIconBuilder::new()
                .icon(handle.default_window_icon().unwrap().clone())
                .menu(&tray_menu)
                .on_menu_event(move |app, event| {
                    let menu_state = handle_clone.state::<MenuState<_>>();
                    let audio_device_map = menu_state.audio_device_map.lock().unwrap();
                    handle_audio_device_selection(app.clone(), &event.id().0, &audio_device_map);
                })
                .build(handle)?;
            
            app.manage(tray);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
