use tauri::{
    AppHandle, Runtime, Manager,
    menu::{Menu, MenuItem, Submenu, CheckMenuItem, PredefinedMenuItem},
    State,
};
use std::sync::Mutex;
use std::collections::HashMap;
use crate::audio::AudioManager;
use crate::config::{ConfigManager, AudioConfig};

#[derive(Default)]
pub struct MenuState<R: Runtime> {
    pub audio_device_map: Mutex<HashMap<String, CheckMenuItem<R>>>,
    pub remove_silence_item: Option<CheckMenuItem<R>>,
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
        _ => {
            eprintln!("Warning: Unhandled menu item: {:?}", id);
        }
    }
}

pub fn create_tray_menu<R: Runtime>(app: &AppHandle<R>) -> (Menu<R>, HashMap<String, CheckMenuItem<R>>, Option<CheckMenuItem<R>>) {
    let separator = PredefinedMenuItem::separator(app).unwrap();
    let quit = MenuItem::with_id(app, "quit", "Quit".to_string(), true, None::<String>).unwrap();

    let config_manager = ConfigManager::<AudioConfig>::new("audio").expect("Failed to create config manager");
    let mut audio_config = AudioConfig::default();
    
    if config_manager.config_exists("audio") {
        match config_manager.load_config("audio") {
            Ok(config) => audio_config = config,
            Err(e) => eprintln!("Failed to load audio configuration: {}", e),
        }
    }

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

    let audio_device_refs: Vec<&dyn tauri::menu::IsMenuItem<R>> = audio_device_items.iter()
        .map(|item| item as &dyn tauri::menu::IsMenuItem<R>)
        .collect();

    let audio_submenu = Submenu::with_items(
        app,
        "Audio Device",
        true,
        &audio_device_refs
    ).unwrap();
    
    let initial_remove_silence_state = audio_config.remove_silence;
    let remove_silence_item = CheckMenuItem::with_id(
        app, 
        "remove_silence", 
        "Remove Silence", 
        true, 
        initial_remove_silence_state, 
        None::<String>
    ).unwrap();
    
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
            if let Ok(current_device) = audio_manager.get_current_device_name() {
                for (device_id, item) in audio_device_map {
                    item.set_checked(device_id == &current_device).unwrap();
                }
            }
        } else {
            for (device_id, item) in audio_device_map {
                item.set_checked(device_id == id).unwrap();
            }

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
