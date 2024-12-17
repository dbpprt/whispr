use tauri::{AppHandle, Runtime, Manager, App};
use std::sync::Mutex;
use crate::{
    audio::AudioManager,
    window::OverlayWindow,
    hotkey::HotkeyManager,
    config::{ConfigManager, AudioConfig},
    menu::{create_tray_menu, MenuState},
};

pub fn initialize_app(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle();
    let (tray_menu, _, menu_state) = create_tray_menu(&app_handle);
    
    app.manage(menu_state);
    
    let mut overlay_window = OverlayWindow::new();
    overlay_window.create_window(&app_handle);
    let overlay_window = Mutex::new(overlay_window);
    app.manage(overlay_window);
    
    let config_manager = ConfigManager::<AudioConfig>::new("audio").expect("Failed to create config manager");
    let mut audio_config = AudioConfig::default();
    
    if config_manager.config_exists("audio") {
        match config_manager.load_config("audio") {
            Ok(config) => audio_config = config,
            Err(e) => eprintln!("Failed to load audio configuration: {}", e),
        }
    }

    let mut audio_manager = AudioManager::new().expect("Failed to initialize audio manager");
    if let Some(device_name) = &audio_config.device_name {
        if let Err(e) = audio_manager.set_input_device(device_name) {
            eprintln!("Failed to set input device from configuration: {}", e);
        }
    }
    audio_manager.set_remove_silence(audio_config.remove_silence);

    let audio_manager = Mutex::new(audio_manager);
    app.manage(audio_manager);
    
    let app_handle_clone = app_handle.clone();
    let mut hotkey_manager = HotkeyManager::new(move |is_speaking| {
        println!("Speaking state changed: {}", is_speaking);
        
        if let Some(window_state) = app_handle_clone.try_state::<Mutex<OverlayWindow>>() {
            let window = window_state.lock().unwrap();
            if is_speaking {
                window.show();
            } else {
                window.hide();
            }
        }

        if let Some(audio_state) = app_handle_clone.try_state::<Mutex<AudioManager>>() {
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

    if let Err(e) = hotkey_manager.start() {
        eprintln!("Failed to start hotkey manager: {}", e);
        return Err(e.into());
    }

    let handle_clone = app_handle.clone();
    let tray = tauri::tray::TrayIconBuilder::new()
        .icon(app_handle.default_window_icon().unwrap().clone())
        .menu(&tray_menu)
        .on_menu_event(move |app, event| {
            println!("Menu item clicked: {:?}", event.id());
            let menu_state = handle_clone.state::<MenuState<_>>();
            crate::menu::handle_menu_event(app, &event.id().0, &menu_state);
        })
        .build(app_handle)?;
    
    app.manage(tray);
    Ok(())
}
