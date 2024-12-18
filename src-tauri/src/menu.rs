use tauri::{
    AppHandle, Manager, Runtime,
    menu::{Menu, MenuItem, Submenu, CheckMenuItem, PredefinedMenuItem},
};
use std::sync::Mutex;
use std::collections::HashMap;
use crate::audio::AudioManager;
use crate::config::{ConfigManager, WhisprConfig};
use tauri_plugin_shell::ShellExt;

type MenuItemMap<R> = HashMap<String, CheckMenuItem<R>>;

#[derive(Default)]
pub struct MenuState<R: Runtime> {
    pub audio_device_map: Mutex<MenuItemMap<R>>,
    pub remove_silence_item: Option<CheckMenuItem<R>>,
    pub save_recordings_item: Option<CheckMenuItem<R>>,
    pub language_items: Mutex<MenuItemMap<R>>,
    pub translate_item: Option<CheckMenuItem<R>>,
    pub start_at_login_item: Option<CheckMenuItem<R>>, // New field for Start at Login
}

impl<R: Runtime> MenuState<R> {
    fn update_config<F>(f: F) -> Result<(), String>
    where
        F: FnOnce(&mut WhisprConfig),
    {
        let config_manager = ConfigManager::<WhisprConfig>::new("settings")
            .map_err(|e| format!("Failed to create config manager: {}", e))?;
        
        let mut config = config_manager.load_config("settings")
            .unwrap_or_default();
        
        f(&mut config);
        
        config_manager.save_config(&config, "settings")
            .map_err(|e| format!("Failed to save configuration: {}", e))
    }

    fn toggle_check_item(item: &CheckMenuItem<R>, f: impl FnOnce(bool) -> Result<(), String>) -> Result<(), String> {
        let new_state = !item.is_checked().map_err(|e| e.to_string())?;
        item.set_checked(new_state).map_err(|e| e.to_string())?;
        f(new_state)
    }
}

pub fn handle_menu_event<R: Runtime>(app: AppHandle<R>, id: &str, menu_state: &MenuState<R>) {
    let result = match id {
        "quit" => {
            app.exit(0);
            Ok(())
        }
        "remove_silence" => {
            menu_state.remove_silence_item.as_ref()
                .map(|item| handle_remove_silence_selection(app.clone(), item))
                .unwrap_or_else(|| Ok(()))
        }
        id if id.starts_with("audio_device_") => {
            id.strip_prefix("audio_device_")
                .map(|device_id| {
                    let audio_device_map = menu_state.audio_device_map.lock().unwrap();
                    handle_audio_device_selection(app.clone(), device_id, &audio_device_map)
                })
                .unwrap_or_else(|| Ok(()))
        }
        "save_recordings" => {
            menu_state.save_recordings_item.as_ref()
                .map(|item| handle_save_recordings_selection(item))
                .unwrap_or_else(|| Ok(()))
        }
        "about" => {
            if let Err(e) = app.shell().command("open")
                .args(&["https://github.com/dbpprt/whispr"])
                .spawn() {
                Err(e.to_string())
            } else {
                Ok(())
            }
        }
        id if id.starts_with("language_") => {
            let language = match id.strip_prefix("language_").unwrap() {
                "Automatic" => "auto",
                "English" => "en",
                "German" => "de",
                "French" => "fr",
                "Spanish" => "es",
                lang => return eprintln!("Error: Unknown language selected: {}", lang),
            };
            let language_items = menu_state.language_items.lock().unwrap();
            if let Some(item) = language_items.get(id) {
                handle_language_selection(app.clone(), item.clone(), language)
            } else {
                Ok(())
            }
        }
        "translate" => {
            menu_state.translate_item.as_ref()
                .map(|item| handle_translate_selection(item))
                .unwrap_or_else(|| Ok(()))
        }
        "start_at_login" => { // New handler for Start at Login
            menu_state.start_at_login_item.as_ref()
                .map(|item| handle_start_at_login_selection(item))
                .unwrap_or_else(|| Ok(()))
        }
        _ => Ok(()),
    };

    if let Err(e) = result {
        eprintln!("Error handling menu event: {}", e);
    }
}

pub fn create_tray_menu<R: Runtime>(app: &AppHandle<R>) -> (Menu<R>, MenuItemMap<R>, MenuState<R>) {
    let config = ConfigManager::<WhisprConfig>::new("settings")
        .and_then(|cm| cm.load_config("settings"))
        .unwrap_or_default();

    let (audio_submenu, audio_device_map) = create_audio_submenu(app, &config);
    let remove_silence_item = create_check_item(app, "remove_silence", "Remove Silence", config.audio.remove_silence);
    let save_recordings_item = create_check_item(app, "save_recordings", "Save Recordings", config.developer.save_recordings);
    let (language_submenu, language_items) = create_language_submenu(app, &config);
    let translate_item = create_check_item(app, "translate", "Translate to English", config.whisper.translate);
    let start_at_login_item = create_check_item(app, "start_at_login", "Start at Login", config.start_at_login); // New item for Start at Login

    let developer_submenu = Submenu::with_items(
        app,
        "Developer Options",
        true,
        &[&save_recordings_item],
    ).unwrap();

    let menu = Menu::with_items(app, &[
        &MenuItem::with_id(app, "quit", "Quit", true, None::<String>).unwrap(),
        &PredefinedMenuItem::separator(app).unwrap(),
        &start_at_login_item, // Added Start at Login item
        &PredefinedMenuItem::separator(app).unwrap(),
        &audio_submenu,
        &language_submenu,
        &translate_item,
        &remove_silence_item,
        &PredefinedMenuItem::separator(app).unwrap(),
        &developer_submenu,
        &MenuItem::with_id(app, "about", "About", true, None::<String>).unwrap(),
    ]).unwrap();

    let menu_state = MenuState {
        audio_device_map: Mutex::new(audio_device_map.clone()),
        remove_silence_item: Some(remove_silence_item),
        save_recordings_item: Some(save_recordings_item),
        language_items: Mutex::new(language_items),
        translate_item: Some(translate_item),
        start_at_login_item: Some(start_at_login_item), // Added Start at Login item to menu state
    };

    (menu, audio_device_map, menu_state)
}

fn create_check_item<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
    label: &str,
    checked: bool,
) -> CheckMenuItem<R> {
    CheckMenuItem::with_id(app, id, label, true, checked, None::<String>).unwrap()
}

fn create_audio_submenu<R: Runtime>(
    app: &AppHandle<R>,
    config: &WhisprConfig,
) -> (Submenu<R>, MenuItemMap<R>) {
    let mut audio_device_map = HashMap::new();
    let audio_manager = AudioManager::new().unwrap();
    
    let items: Vec<CheckMenuItem<R>> = audio_manager.list_input_devices()
        .unwrap_or_default()
        .into_iter()
        .map(|device| {
            let is_active = config.audio.device_name.as_ref().map_or(false, |d| d == &device);
            let item_id = format!("audio_device_{}", device);
            let item = create_check_item(app, &item_id, &device, is_active);
            audio_device_map.insert(device, item.clone());
            item
        })
        .collect();

    let submenu = Submenu::with_items(
        app,
        "Audio Device",
        true,
        &items.iter().map(|i| i as &dyn tauri::menu::IsMenuItem<R>).collect::<Vec<_>>(),
    ).unwrap();

    (submenu, audio_device_map)
}

fn create_language_submenu<R: Runtime>(
    app: &AppHandle<R>,
    config: &WhisprConfig,
) -> (Submenu<R>, MenuItemMap<R>) {
    let languages = [
        ("Automatic", "auto"),
        ("English", "en"),
        ("German", "de"),
        ("French", "fr"),
        ("Spanish", "es"),
    ];

    let mut language_items = HashMap::new();
    let items: Vec<CheckMenuItem<R>> = languages
        .iter()
        .map(|(label, code)| {
            let is_active = config.whisper.language.as_deref() == Some(code);
            let item_id = format!("language_{}", label);
            let item = create_check_item(app, &item_id, label, is_active);
            language_items.insert(item_id, item.clone());
            item
        })
        .collect();

    let submenu = Submenu::with_items(
        app,
        "Language",
        true,
        &items.iter().map(|i| i as &dyn tauri::menu::IsMenuItem<R>).collect::<Vec<_>>(),
    ).unwrap();

    (submenu, language_items)
}

fn handle_audio_device_selection<R: Runtime>(
    app: AppHandle<R>,
    id: &str,
    audio_device_map: &MenuItemMap<R>,
) -> Result<(), String> {
    let audio_state = app.try_state::<Mutex<AudioManager>>()
        .ok_or("Audio manager not found")?;
    let mut audio_manager = audio_state.lock().unwrap();

    if let Err(e) = audio_manager.set_input_device(id) {
        return Err(e.to_string());
    }

    for (device_id, item) in audio_device_map {
        item.set_checked(device_id == id)
            .map_err(|e| e.to_string())?;
    }

    MenuState::<R>::update_config(|config| {
        config.audio.device_name = Some(id.to_string());
    })
}

fn handle_remove_silence_selection<R: Runtime>(
    app: AppHandle<R>,
    item: &CheckMenuItem<R>,
) -> Result<(), String> {
    let audio_state = app.try_state::<Mutex<AudioManager>>()
        .ok_or("Audio manager not found")?;
    
    MenuState::<R>::toggle_check_item(item, |new_state| {
        let mut audio_manager = audio_state.lock().unwrap();
        audio_manager.set_remove_silence(new_state);
        
        MenuState::<R>::update_config(|config| {
            config.audio.remove_silence = new_state;
        })
    })
}

fn handle_save_recordings_selection<R: Runtime>(
    item: &CheckMenuItem<R>,
) -> Result<(), String> {
    MenuState::<R>::toggle_check_item(item, |new_state| {
        MenuState::<R>::update_config(|config| {
            config.developer.save_recordings = new_state;
        })
    })
}

fn handle_language_selection<R: Runtime>(
    app: AppHandle<R>,
    item: CheckMenuItem<R>,
    language: &str,
) -> Result<(), String> {
    MenuState::<R>::update_config(|config| {
        config.whisper.language = Some(language.to_string());
    })?;

    let menu_state = app.state::<MenuState<R>>();
    let mut language_items = menu_state.language_items.lock().unwrap();
    
    for (item_id, menu_item) in &mut *language_items {
        menu_item.set_checked(item_id.strip_prefix("language_").unwrap() == language)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn handle_translate_selection<R: Runtime>(
    item: &CheckMenuItem<R>,
) -> Result<(), String> {
    MenuState::<R>::toggle_check_item(item, |new_state| {
        MenuState::<R>::update_config(|config| {
            config.whisper.translate = new_state;
        })
    })
}

fn handle_start_at_login_selection<R: Runtime>(item: &CheckMenuItem<R>) -> Result<(), String> { // New handler for Start at Login
    MenuState::<R>::toggle_check_item(item, |new_state| {
        MenuState::<R>::update_config(|config| {
            config.start_at_login = new_state;
        })
    })
}
