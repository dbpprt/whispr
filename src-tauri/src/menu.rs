use tauri::{
    AppHandle, Manager, Runtime,
    menu::{Menu, MenuItem, Submenu, CheckMenuItem, PredefinedMenuItem},
};
use std::sync::Mutex;
use std::collections::HashMap;
use crate::audio::AudioManager;
use crate::config::{ConfigManager, WhisprConfig};
use tauri_plugin_shell::ShellExt;

#[derive(Default)]
pub struct MenuState<R: Runtime> {
    pub audio_device_map: Mutex<HashMap<String, CheckMenuItem<R>>>,
    pub remove_silence_item: Option<CheckMenuItem<R>>,
    pub save_recordings_item: Option<CheckMenuItem<R>>,
}

pub fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, id: &str, menu_state: &MenuState<R>) {
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
        "save_recordings" => {
            if let Some(save_recordings_item) = &menu_state.save_recordings_item {
                handle_save_recordings_selection(app.clone(), save_recordings_item);
            }
        }
        "about" => {
            let _ = app.shell().command("open")
                .args(&["https://github.com/dbpprt/whispr"])
                .spawn();
        }
        _ => {
            eprintln!("Warning: Unhandled menu item: {:?}", id);
        }
    }
}

pub fn create_tray_menu<R: Runtime>(app: &AppHandle<R>) -> (Menu<R>, HashMap<String, CheckMenuItem<R>>, MenuState<R>) {
    let separator = PredefinedMenuItem::separator(app).unwrap();
    let quit = MenuItem::with_id(app, "quit", "Quit".to_string(), true, None::<String>).unwrap();

    let config_manager = ConfigManager::<WhisprConfig>::new("settings").expect("Failed to create config manager");
    let mut whispr_config = WhisprConfig::default();
    
    if config_manager.config_exists("settings") {
        match config_manager.load_config("settings") {
            Ok(config) => whispr_config = config,
            Err(e) => eprintln!("Failed to load configuration: {}", e),
        }
    }

    let mut audio_device_items = Vec::new();
    let mut audio_device_map = HashMap::new();
    let audio_manager = AudioManager::new().unwrap();
    
    if let Ok(devices) = audio_manager.list_input_devices() {
        for device in devices {
            let is_active = whispr_config.audio.device_name.as_ref().map_or(false, |d| d == &device);
            let item_id = format!("audio_device_{}", device);
            let item = CheckMenuItem::with_id(app, &item_id, &device, true, is_active, None::<String>).unwrap();
            audio_device_items.push(item.clone());
            audio_device_map.insert(device.to_string(), item);
        }
    } else {
        eprintln!("Failed to get list of input devices");
    }

    let audio_device_refs: Vec<&dyn tauri::menu::IsMenuItem<R>> = audio_device_items.iter()
        .map(|item| item as &dyn tauri::menu::IsMenuItem<R>)
        .collect();

    let audio_submenu = Submenu::with_items(
        app,
        "Audio Device",
        true,
        &audio_device_refs
    ).unwrap();
    
    let initial_remove_silence_state = whispr_config.audio.remove_silence;
    let remove_silence_item = CheckMenuItem::with_id(
        app, 
        "remove_silence", 
        "Remove Silence", 
        true, 
        initial_remove_silence_state, 
        None::<String>
    ).unwrap();
    
    let developer_options_separator = PredefinedMenuItem::separator(app).unwrap();

    let save_recordings_item = CheckMenuItem::with_id(
        app,
        "save_recordings",
        "Save Recordings",
        true,
        whispr_config.developer.save_recordings,
        None::<String>
    ).unwrap();
    
    let developer_options_submenu = Submenu::with_items(
        app,
        "Developer Options",
        true,
        &[&save_recordings_item as &dyn tauri::menu::IsMenuItem<R>]
    ).unwrap();
    
    let about = MenuItem::with_id(app, "about", "About".to_string(), true, None::<String>).unwrap();

    let main_items: Vec<&dyn tauri::menu::IsMenuItem<R>> = vec![
        &quit,
        &separator,
        &audio_submenu,
        &remove_silence_item,
        &developer_options_separator,
        &developer_options_submenu,
        &about,
    ];

    let menu = Menu::with_items(app, &main_items).unwrap();
    let menu_state = MenuState {
        audio_device_map: Mutex::new(audio_device_map.clone()),
        remove_silence_item: Some(remove_silence_item),
        save_recordings_item: Some(save_recordings_item),
    };
    
    (menu, audio_device_map.clone(), menu_state)
}

fn handle_audio_device_selection<R: Runtime>(app: AppHandle<R>, id: &str, audio_device_map: &HashMap<String, CheckMenuItem<R>>) {
    if let Some(audio_state) = app.try_state::<Mutex<AudioManager>>() {
        let mut audio_manager = audio_state.lock().unwrap();
        if let Err(e) = audio_manager.set_input_device(id) {
            eprintln!("Failed to set input device: {}", e);
            if let Ok(current_device) = audio_manager.get_current_device_name() {
                for (device_id, item) in audio_device_map {
                    item.set_checked(device_id == &current_device).unwrap();
                }
            }
        } else {
            for (device_id, item) in audio_device_map {
                item.set_checked(device_id == id).unwrap();
            }

            let config_manager = ConfigManager::<WhisprConfig>::new("settings").expect("Failed to create config manager");
            let mut whispr_config = WhisprConfig::default();
            if let Ok(config) = config_manager.load_config("settings") {
                whispr_config = config;
            }
            whispr_config.audio.device_name = Some(id.to_string());
            if let Err(e) = config_manager.save_config(&whispr_config, "settings") {
                eprintln!("Failed to save configuration: {}", e);
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

        let config_manager = ConfigManager::<WhisprConfig>::new("settings").expect("Failed to create config manager");
        let mut whispr_config = WhisprConfig::default();
        if let Ok(config) = config_manager.load_config("settings") {
            whispr_config = config;
        }
        whispr_config.audio.remove_silence = new_state;
        if let Err(e) = config_manager.save_config(&whispr_config, "settings") {
            eprintln!("Failed to save configuration: {}", e);
        }
    }
}

fn handle_save_recordings_selection<R: Runtime>(app: AppHandle<R>, save_recordings_item: &CheckMenuItem<R>) {
    let config_manager = ConfigManager::<WhisprConfig>::new("settings").expect("Failed to create config manager");
    let mut whispr_config = WhisprConfig::default();
    
    if config_manager.config_exists("settings") {
        match config_manager.load_config("settings") {
            Ok(config) => whispr_config = config,
            Err(e) => eprintln!("Failed to load configuration: {}", e),
        }
    }

    let current_state = whispr_config.developer.save_recordings;
    let new_state = !current_state;

    println!("Save Recordings before toggle: {}", current_state);
    save_recordings_item.set_checked(new_state).unwrap();
    println!("Save Recordings after toggle: {}", new_state);

    whispr_config.developer.save_recordings = new_state;
    if let Err(e) = config_manager.save_config(&whispr_config, "settings") {
        eprintln!("Failed to save configuration: {}", e);
    }
}
