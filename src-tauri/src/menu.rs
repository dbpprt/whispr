use tauri::{
    AppHandle, Manager, Runtime,
    menu::{Menu, MenuItem, Submenu, CheckMenuItem, PredefinedMenuItem},
};
use std::collections::HashMap;
use crate::audio::AudioManager;
use crate::config::{ConfigManager, WhisprConfig};
use tauri_plugin_shell::ShellExt;
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons}; // Added import for tauri_plugin_dialog

#[derive(Default)]
pub struct MenuState<R: Runtime> {
    pub audio_device_map: HashMap<String, CheckMenuItem<R>>,
    pub remove_silence_item: Option<CheckMenuItem<R>>,
    pub save_recordings_item: Option<CheckMenuItem<R>>,
    pub language_items: HashMap<String, CheckMenuItem<R>>,
    pub translate_item: Option<CheckMenuItem<R>>,
    pub start_at_login_item: Option<CheckMenuItem<R>>,
    pub whisper_logging_item: Option<CheckMenuItem<R>>, // New field for Whisper logging
    pub keyboard_shortcut_items: HashMap<String, CheckMenuItem<R>>, // New field for Keyboard Shortcut items
}

pub fn handle_menu_event<R: Runtime>(app: AppHandle<R>, id: &str, menu_state: &MenuState<R>) {
    match id {
        "quit" => {
            println!("Quit menu item selected");
            app.exit(0);
        }
        "remove_silence" => {
            if let Some(remove_silence_item) = &menu_state.remove_silence_item {
                handle_remove_silence_selection(&app, remove_silence_item);
            }
        }
        id if id.starts_with("audio_device_") => {
            if let Some(device_id) = id.strip_prefix("audio_device_") {
                handle_audio_device_selection(&app, device_id, &menu_state.audio_device_map);
            } else {
                eprintln!("Warning: Invalid audio device ID format: {:?}", id);
            }
        }
        "save_recordings" => {
            if let Some(save_recordings_item) = &menu_state.save_recordings_item {
                handle_save_recordings_selection(&app, save_recordings_item);
            }
        }
        "about" => {
            let _ = app.shell().command("open")
                .args(&["https://github.com/dbpprt/whispr"])
                .spawn();
        }
        id if id.starts_with("language_") => {
            if let Some(item) = menu_state.language_items.get(id) {
                let language = match id.strip_prefix("language_").unwrap() {
                    "Automatic" => "auto",
                    "English" => "en",
                    "German" => "de",
                    "French" => "fr",
                    "Spanish" => "es",
                    _ => {
                        eprintln!("Error: Unknown language selected: {}", id);
                        return;
                    }
                };
                handle_language_selection(&app, item.clone(), language);
            }
        }
        "translate" => {
            if let Some(translate_item) = &menu_state.translate_item {
                handle_translate_selection(&app, translate_item);
            }
        }
        "start_at_login" => {
            if let Some(start_at_login_item) = &menu_state.start_at_login_item {
                handle_start_at_login_selection(&app, start_at_login_item);
            }
        }
        "whisper_logging" => { // New event handler for Whisper logging
            if let Some(whisper_logging_item) = &menu_state.whisper_logging_item {
                handle_whisper_logging_selection(&app, whisper_logging_item);
            }
        }
        id if id.starts_with("keyboard_shortcut_") => {
            if let Some(item) = menu_state.keyboard_shortcut_items.get(id) {
                let shortcut = match id.strip_prefix("keyboard_shortcut_").unwrap() {
                    "right_option_key" => "right_option_key",
                    "right_command_key" => "right_command_key",
                    _ => {
                        eprintln!("Error: Unknown keyboard shortcut selected: {}", id);
                        return;
                    }
                };
                handle_keyboard_shortcut_selection(&app, item.clone(), shortcut);
            }
        }
        "restart" => {
            app.restart();
        }
        _ => {
            eprintln!("Warning: Unhandled menu item: {:?}", id);
        }
    }
}

pub fn create_tray_menu<R: Runtime>(app: &AppHandle<R>) -> (Menu<R>, MenuState<R>) {
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
    
    let whisper_logging_item = CheckMenuItem::with_id( // New item for Whisper logging
        app,
        "whisper_logging",
        "Whisper Logging",
        true,
        whispr_config.developer.whisper_logging,
        None::<String>
    ).unwrap();

    let restart = MenuItem::with_id(app, "restart", "Restart".to_string(), true, None::<String>).unwrap();

    let developer_options_submenu = Submenu::with_items(
        app,
        "Developer Options",
        true,
        &[
            &save_recordings_item as &dyn tauri::menu::IsMenuItem<R>,
            &whisper_logging_item as &dyn tauri::menu::IsMenuItem<R>,
            &restart as &dyn tauri::menu::IsMenuItem<R>
        ]
    ).unwrap();

    let language_items = vec![
        ("Automatic", whispr_config.whisper.language.as_ref().map_or(true, |l| l == "auto")),
        ("English", whispr_config.whisper.language.as_ref().map_or(false, |l| l == "en")),
        ("German", whispr_config.whisper.language.as_ref().map_or(false, |l| l == "de")),
        ("French", whispr_config.whisper.language.as_ref().map_or(false, |l| l == "fr")),
        ("Spanish", whispr_config.whisper.language.as_ref().map_or(false, |l| l == "es")),
    ];

    let mut language_check_items = HashMap::new();
    let mut language_menu_items: Vec<&'static dyn tauri::menu::IsMenuItem<R>> = Vec::new();

    for (language, is_active) in language_items {
        let item_id = format!("language_{}", language);
        let item = CheckMenuItem::with_id(app, &item_id, language, true, is_active, None::<String>).unwrap();
        language_check_items.insert(item_id.clone(), item.clone());
        language_menu_items.push(Box::leak(Box::new(item)) as &'static dyn tauri::menu::IsMenuItem<R>);
    }

    let language_submenu = Submenu::with_items(
        app,
        "Language",
        true,
        &language_menu_items
    ).unwrap();

    let translate_item = CheckMenuItem::with_id(
        app,
        "translate",
        "Translate to English",
        true,
        whispr_config.whisper.translate,
        None::<String>
    ).unwrap();

    let start_at_login_item = CheckMenuItem::with_id(
        app,
        "start_at_login",
        "Start at Login",
        true,
        whispr_config.start_at_login,
        None::<String>
    ).unwrap();

    let keyboard_shortcut_items = vec![
        ("Right Option Key", whispr_config.keyboard_shortcut == "right_option_key"),
        ("Right Command Key", whispr_config.keyboard_shortcut == "right_command_key"),
    ];

    let mut keyboard_shortcut_check_items = HashMap::new();
    let mut keyboard_shortcut_menu_items: Vec<&'static dyn tauri::menu::IsMenuItem<R>> = Vec::new();

    for (shortcut, is_active) in keyboard_shortcut_items {
        let item_id = format!("keyboard_shortcut_{}", shortcut.to_lowercase().replace(' ', "_"));
        let item = CheckMenuItem::with_id(app, &item_id, shortcut, true, is_active, None::<String>).unwrap();
        keyboard_shortcut_check_items.insert(item_id.clone(), item.clone());
        keyboard_shortcut_menu_items.push(Box::leak(Box::new(item)) as &'static dyn tauri::menu::IsMenuItem<R>);
    }

    let keyboard_shortcut_submenu = Submenu::with_items(
        app,
        "Keyboard Shortcut",
        true,
        &keyboard_shortcut_menu_items
    ).unwrap();

    let about = MenuItem::with_id(app, "about", "About".to_string(), true, None::<String>).unwrap();

    let main_items: Vec<&dyn tauri::menu::IsMenuItem<R>> = vec![
        &quit,
        &separator,
        &start_at_login_item,
        &keyboard_shortcut_submenu,
        &separator,
        &audio_submenu,
        &language_submenu,
        &translate_item,
        &remove_silence_item,
        &developer_options_separator,
        &developer_options_submenu,
        &about,
    ];

    let menu = Menu::with_items(app, &main_items).unwrap();
    let menu_state = MenuState {
        audio_device_map,
        remove_silence_item: Some(remove_silence_item),
        save_recordings_item: Some(save_recordings_item),
        language_items: language_check_items,
        translate_item: Some(translate_item),
        start_at_login_item: Some(start_at_login_item),
        whisper_logging_item: Some(whisper_logging_item), // Include Whisper logging item in state
        keyboard_shortcut_items: keyboard_shortcut_check_items,
    };
    
    (menu, menu_state)
}

fn handle_audio_device_selection<R: Runtime>(app: &AppHandle<R>, id: &str, audio_device_map: &HashMap<String, CheckMenuItem<R>>) {
    if let Some(app_state) = app.try_state::<crate::AppState>() {
        let mut audio_manager = app_state.audio.lock().unwrap();
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

fn handle_remove_silence_selection<R: Runtime>(app: &AppHandle<R>, remove_silence_item: &CheckMenuItem<R>) {
    if let Some(app_state) = app.try_state::<crate::AppState>() {
        let mut audio_manager = app_state.audio.lock().unwrap();
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

fn handle_save_recordings_selection<R: Runtime>(_app: &AppHandle<R>, save_recordings_item: &CheckMenuItem<R>) {
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

fn handle_whisper_logging_selection<R: Runtime>(_app: &AppHandle<R>, whisper_logging_item: &CheckMenuItem<R>) { // New function for Whisper logging
    let config_manager = ConfigManager::<WhisprConfig>::new("settings").expect("Failed to create config manager");
    let mut whispr_config = WhisprConfig::default();
    
    if config_manager.config_exists("settings") {
        match config_manager.load_config("settings") {
            Ok(config) => whispr_config = config,
            Err(e) => eprintln!("Failed to load configuration: {}", e),
        }
    }

    let current_state = whispr_config.developer.whisper_logging;
    let new_state = !current_state;

    println!("Whisper Logging before toggle: {}", current_state);
    whisper_logging_item.set_checked(new_state).unwrap();
    println!("Whisper Logging after toggle: {}", new_state);

    whispr_config.developer.whisper_logging = new_state;
    if let Err(e) = config_manager.save_config(&whispr_config, "settings") {
        eprintln!("Failed to save configuration: {}", e);
    }
}

fn handle_language_selection<R: Runtime>(app: &AppHandle<R>, item: CheckMenuItem<R>, language: &str) {
    println!("handle_language_selection called with language: {}", language);
    let config_manager = ConfigManager::<WhisprConfig>::new("settings").expect("Failed to create config manager");
    println!("ConfigManager created");
    let mut whispr_config = WhisprConfig::default();
    println!("WhisprConfig initialized");

    if config_manager.config_exists("settings") {
        match config_manager.load_config("settings") {
            Ok(config) => {
                whispr_config = config;
                println!("Configuration loaded successfully");
            }
            Err(e) => {
                eprintln!("Failed to load configuration: {}", e);
                return;
            }
        }
    }

    whispr_config.whisper.language = Some(language.to_string());
    println!("Language updated to: {}", language);
    if let Err(e) = config_manager.save_config(&whispr_config, "settings") {
        eprintln!("Failed to save configuration: {}", e);
        return;
    }

    let menu_state = app.state::<MenuState<R>>();
    for (item_id, menu_item) in &menu_state.language_items {
        menu_item.set_checked(item_id.strip_prefix("language_").unwrap() == language).unwrap();
    }
    println!("Menu items updated");
}

fn handle_translate_selection<R: Runtime>(_app: &AppHandle<R>, translate_item: &CheckMenuItem<R>) {
    let config_manager = ConfigManager::<WhisprConfig>::new("settings").expect("Failed to create config manager");
    let mut whispr_config = WhisprConfig::default();
    
    if config_manager.config_exists("settings") {
        match config_manager.load_config("settings") {
            Ok(config) => whispr_config = config,
            Err(e) => eprintln!("Failed to save configuration: {}", e),
        }
    }

    let current_state = whispr_config.whisper.translate;
    let new_state = !current_state;

    println!("Translate before toggle: {}", current_state);
    translate_item.set_checked(new_state).unwrap();
    println!("Translate after toggle: {}", new_state);

    whispr_config.whisper.translate = new_state;
    if let Err(e) = config_manager.save_config(&whispr_config, "settings") {
        eprintln!("Failed to save configuration: {}", e);
    }
}

fn handle_start_at_login_selection<R: Runtime>(app: &AppHandle<R>, start_at_login_item: &CheckMenuItem<R>) {
    println!("Start at login selection handler called");
    
    let config_manager = ConfigManager::<WhisprConfig>::new("settings").expect("Failed to create config manager");
    let mut whispr_config = WhisprConfig::default();
    
    if config_manager.config_exists("settings") {
        match config_manager.load_config("settings") {
            Ok(config) => whispr_config = config,
            Err(e) => eprintln!("Failed to load configuration: {}", e),
        }
    }

    let current_state = whispr_config.start_at_login;
    let new_state = !current_state;

    println!("Start at login before toggle: {}", current_state);
    
    let autolaunch = app.autolaunch();
    let autolaunch_result = if new_state {
        println!("Enabling autolaunch");
        autolaunch.enable()
    } else {
        println!("Disabling autolaunch");
        autolaunch.disable()
    };

    if let Err(e) = autolaunch_result {
        eprintln!("Failed to update autolaunch state: {}", e);
        return;
    }

    if let Err(e) = start_at_login_item.set_checked(new_state) {
        eprintln!("Failed to update checkbox state: {}", e);
        return;
    }

    println!("Start at login after toggle: {}", new_state);

    whispr_config.start_at_login = new_state;
    if let Err(e) = config_manager.save_config(&whispr_config, "settings") {
        eprintln!("Failed to save configuration: {}", e);
    }
}

fn handle_keyboard_shortcut_selection<R: Runtime>(app: &AppHandle<R>, item: CheckMenuItem<R>, shortcut: &str) {
    let config_manager = ConfigManager::<WhisprConfig>::new("settings").expect("Failed to create config manager");
    let mut whispr_config = WhisprConfig::default();
    
    if config_manager.config_exists("settings") {
        match config_manager.load_config("settings") {
            Ok(config) => whispr_config = config,
            Err(e) => eprintln!("Failed to load configuration: {}", e),
        }
    }

    let target_shortcut = shortcut.to_string();
    let app_handle = app.clone();
    let current_shortcut = whispr_config.keyboard_shortcut.clone();

    app.dialog()
        .message("Application must be restarted for changes to take effect")
        .title("Restart Required")
        .buttons(MessageDialogButtons::OkCancel)
        .show(move |answer| {
            if answer {
                let mut config = whispr_config.clone();
                config.keyboard_shortcut = target_shortcut.clone();
                
                if let Err(e) = config_manager.save_config(&config, "settings") {
                    eprintln!("Failed to save configuration: {}", e);
                    return;
                }

                let menu_state = app_handle.state::<MenuState<R>>();
                for (item_id, menu_item) in &menu_state.keyboard_shortcut_items {
                    menu_item.set_checked(item_id.strip_prefix("keyboard_shortcut_").unwrap() == target_shortcut).unwrap();
                }

                // Restart the app
                app_handle.restart();
            } else {
                // Revert the menu item state to the previous shortcut
                let menu_state = app_handle.state::<MenuState<R>>();
                for (item_id, menu_item) in &menu_state.keyboard_shortcut_items {
                    menu_item.set_checked(item_id.strip_prefix("keyboard_shortcut_").unwrap() == current_shortcut).unwrap();
                }
            }
        });
}
