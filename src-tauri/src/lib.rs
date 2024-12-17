use tauri::{
    AppHandle, Manager, Runtime,
    menu::{Menu, MenuItem, Submenu, CheckMenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    State,
};
mod hotkey;
mod window;
mod audio;
mod config; // Declare the config module

use hotkey::HotkeyManager;
use window::OverlayWindow;
use audio::AudioManager;
use std::sync::Mutex;
use std::collections::HashMap;
use config::{ConfigManager, AudioConfig};

// State struct to hold menu items
#[derive(Default)]
struct MenuState<R: Runtime> {
    audio_device_map: Mutex<HashMap<String, CheckMenuItem<R>>>,
    remove_silence_item: Option<CheckMenuItem<R>>,
}

fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, id: &str, menu_state: &MenuState<R>) {
    match id {
        "quit" => {
            println!("Quit menu item selected");
            app.exit(0);
        }
        "remove_silence" => {
            if let Some(remove_silence_item) = &menu_state.remove_silence_item {
                handle_remove_silence_selection(app.clone(), remove_silence_item);
            }
        }
        id if id.starts_with("audio_device_") => {
            let audio_device_map = menu_state.audio_device_map.lock().unwrap();
            if let Some(device_id) = id.strip_prefix("audio_device_") {
                handle_audio_device_selection(app.clone(), device_id, &audio_device_map);
            } else {
                eprintln!("Warning: Invalid audio device ID format: {:?}", id);
            }
        }
        _ => {
            eprintln!("Warning: Unhandled menu item: {:?}", id);
        }
    }
}

fn create_tray_menu<R: Runtime>(app: &AppHandle<R>) -> (Menu<R>, HashMap<String, CheckMenuItem<R>>, Option<CheckMenuItem<R>>) {
    // Create quit menu item
    let separator = PredefinedMenuItem::separator(app).unwrap();
    let quit = MenuItem::with_id(app, "quit", "Quit".to_string(), true, None::<String>).unwrap();

    // Initialize config manager and load audio configuration
    let config_manager = ConfigManager::<AudioConfig>::new("audio").expect("Failed to create config manager");
    let mut audio_config = AudioConfig::default();
    
    if config_manager.config_exists("audio") {
        match config_manager.load_config("audio") {
            Ok(config) => audio_config = config,
            Err(e) => eprintln!("Failed to load audio configuration: {}", e),
        }
    }

    // Create audio device menu items
    let mut audio_device_items = Vec::new();
    let mut audio_device_map = HashMap::new();
    let audio_manager = AudioManager::new().unwrap();
    
    if let Ok(devices) = audio_manager.list_input_devices() {
        for device in devices {
            let is_active = audio_config.device_name.as_ref().map_or(false, |d| d == &device);
            let item_id = format!("audio_device_{}", device);
            let item = CheckMenuItem::with_id(app, &item_id, &device, true, is_active, None::<String>).unwrap();
            audio_device_items.push(item.clone());
            audio_device_map.insert(device.to_string(), item);
        }
    } else {
        eprintln!("Failed to get list of input devices");
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
    
    // Create remove silence menu item with explicit ID and sync initial state with audio manager
    let initial_remove_silence_state = audio_config.remove_silence;
    let remove_silence_item = CheckMenuItem::with_id(
        app, 
        "remove_silence", 
        "Remove Silence", 
        true, 
        initial_remove_silence_state, 
        None::<String>
    ).unwrap();
    
    // Create the main menu with all items
    let main_items: Vec<&dyn tauri::menu::IsMenuItem<R>> = vec![
        &quit,
        &separator,
        &audio_submenu,
        &remove_silence_item
    ];
    (Menu::with_items(app, &main_items).unwrap(), audio_device_map, Some(remove_silence_item))
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

            // Save the new audio configuration
            let config_manager = ConfigManager::<AudioConfig>::new("audio").expect("Failed to create config manager");
            let mut audio_config = AudioConfig::default();
            audio_config.device_name = Some(id.to_string());
            if let Err(e) = config_manager.save_config(&audio_config, "audio") {
                eprintln!("Failed to save audio configuration: {}", e);
            }
        }
    }
}

fn handle_remove_silence_selection<R: Runtime>(app: AppHandle<R>, remove_silence_item: &CheckMenuItem<R>) {
    if let Some(audio_state) = app.try_state::<Mutex<AudioManager>>() {
        let mut audio_manager = audio_state.lock().unwrap();
        let current_state = audio_manager.is_silence_removal_enabled();
        let new_state = !current_state;
        
        println!("Remove Silence before toggle: {}", current_state);
        audio_manager.set_remove_silence(new_state);
        remove_silence_item.set_checked(new_state).unwrap();
        println!("Remove Silence after toggle: {}", new_state);

        // Save the new audio configuration
        let config_manager = ConfigManager::<AudioConfig>::new("audio").expect("Failed to create config manager");
        let mut audio_config = AudioConfig::default();
        if let Err(e) = config_manager.load_config("audio") {
            eprintln!("Failed to load audio configuration: {}", e);
        } else {
            match config_manager.load_config("audio") {
                Ok(config) => audio_config = config,
                Err(e) => eprintln!("Failed to load audio configuration: {}", e),
            }
        }
        audio_config.remove_silence = new_state;
        if let Err(e) = config_manager.save_config(&audio_config, "audio") {
            eprintln!("Failed to save audio configuration: {}", e);
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let handle = app.handle();
            let (tray_menu, audio_device_map, remove_silence_item) = create_tray_menu(&handle);
            
            // Store menu state
            app.manage(MenuState { 
                audio_device_map: Mutex::new(audio_device_map),
                remove_silence_item,
            });
            
            // Create and store the overlay window
            let mut overlay_window = OverlayWindow::new();
            overlay_window.create_window(&handle);
            let overlay_window = Mutex::new(overlay_window);
            app.manage(overlay_window);
            
            // Initialize config manager
            let config_manager = ConfigManager::<AudioConfig>::new("audio").expect("Failed to create config manager");
            let mut audio_config = AudioConfig::default();
            
            // Load existing configuration if available
            if config_manager.config_exists("audio") {
                match config_manager.load_config("audio") {
                    Ok(config) => audio_config = config,
                    Err(e) => eprintln!("Failed to load audio configuration: {}", e),
                }
            }

            // Initialize audio manager with loaded settings
            let mut audio_manager = AudioManager::new().expect("Failed to initialize audio manager");
            if let Some(device_name) = &audio_config.device_name {
                if let Err(e) = audio_manager.set_input_device(device_name) {
                    eprintln!("Failed to set input device from configuration: {}", e);
                }
            }
            audio_manager.set_remove_silence(audio_config.remove_silence);

            // Store the audio manager
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
                    println!("Menu item clicked: {:?}", event.id());
                    let menu_state = handle_clone.state::<MenuState<_>>();
                    handle_menu_event(&app.clone(), &event.id().0, &menu_state);
                })
                .build(handle)?;
            
            app.manage(tray);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
