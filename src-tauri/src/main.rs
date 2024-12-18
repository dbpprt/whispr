// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod hotkey;
mod window;
mod audio;
mod config;
mod menu;
mod whisper;

use tauri::{Manager, App, AppHandle, Runtime, Wry, Emitter};
use std::sync::{Arc, Mutex};
use std::path::Path;
use enigo::{Enigo, Key, Keyboard, Settings};
use crate::{
    audio::AudioManager,
    window::OverlayWindow,
    hotkey::HotkeyManager,
    config::{ConfigManager, WhisprConfig},
    menu::{create_tray_menu, MenuState},
    whisper::WhisperProcessor,
};

#[derive(thiserror::Error, Debug)]
pub enum WhisprError {
    #[error("Audio initialization failed: {0}")]
    AudioError(String),
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Hotkey error: {0}")]
    HotkeyError(String),
    #[error("Whisper model error: {0}")]
    WhisperError(String),
    #[error("System error: {0}")]
    SystemError(String),
}

type Result<T> = std::result::Result<T, WhisprError>;

struct AppState {
    whisper: WhisperProcessor,
    audio: Mutex<AudioManager>,
    overlay: Mutex<OverlayWindow>,
}

impl AppState {
    fn new(config: WhisprConfig) -> Result<Self> {
        let audio_manager = AudioManager::new()
            .map_err(|e| WhisprError::AudioError(e.to_string()))?;
        
        let model_path = Path::new("/Users/dbpprt/Downloads/ggml-large-v3-turbo.bin");
        let whisper = WhisperProcessor::new(model_path, config)
            .map_err(|e| WhisprError::WhisperError(e))?;
 
        Ok(Self {
            whisper,
            audio: Mutex::new(audio_manager),
            overlay: Mutex::new(OverlayWindow::new()),
        })
    }

    fn configure_audio(&self, config: &WhisprConfig) -> Result<()> {
        let mut audio = self.audio.lock().unwrap();
        if let Some(device_name) = &config.audio.device_name {
            audio.set_input_device(device_name)
                .map_err(|e| WhisprError::AudioError(e.to_string()))?;
        }
        audio.set_remove_silence(config.audio.remove_silence);
        Ok(())
    }
}

fn setup_app(app: &mut App<Wry>) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle();
    
    // Initialize configuration
    let config_manager = ConfigManager::<WhisprConfig>::new("settings")
        .map_err(|e| WhisprError::ConfigError(e.to_string()))?;
    
    let whispr_config = if config_manager.config_exists("settings") {
        config_manager.load_config("settings")
            .map_err(|e| WhisprError::ConfigError(e.to_string()))?
    } else {
        WhisprConfig::default()
    };

    // Initialize application state
    let state = AppState::new(whispr_config.clone())?;
    state.configure_audio(&whispr_config)?;
    
    // Create window
    state.overlay.lock().unwrap().create_window(&app_handle);
    
    // Store state
    app.manage(state);

    // Setup tray and menu
    let (tray_menu, _, menu_state) = create_tray_menu(&app_handle);
    app.manage(menu_state);

    let handle_clone = app_handle.clone();
    let tray = tauri::tray::TrayIconBuilder::new()
        .icon(app_handle.default_window_icon().unwrap().clone())
        .menu(&tray_menu)
        .on_menu_event(move |app, event| {
            let menu_state = handle_clone.state::<MenuState<_>>();
            crate::menu::handle_menu_event(app.clone(), &event.id().0, &menu_state);
        })
        .build(app_handle)
        .map_err(|e| Box::new(WhisprError::SystemError(e.to_string())) as Box<dyn std::error::Error>)?;
    
    app.manage(tray);

    // Setup hotkey manager
    let app_handle_clone = app_handle.clone();
    let mut hotkey_manager = HotkeyManager::new(move |is_speaking| {
        if let Some(state) = app_handle_clone.try_state::<AppState>() {
            let overlay = state.overlay.lock().unwrap();
            if is_speaking {
                overlay.show();
            } else {
                overlay.hide();
            }

            let mut audio = state.audio.lock().unwrap();
            if is_speaking {
                if let Err(e) = audio.start_capture() {
                    eprintln!("Failed to start audio capture: {}", e);
                    return;
                }
                let _ = app_handle_clone.emit("status-change", "Listening");
            } else {
                audio.stop_capture();
                let _ = app_handle_clone.emit("status-change", "Transcribing");
                
                if let Some(captured_audio) = audio.get_captured_audio(16000, 1) {
                    if let Ok(segments) = state.whisper.process_audio(captured_audio) {
                        let transcription: String = segments.iter().map(|(_, _, segment)| segment.clone()).collect::<Vec<String>>().join(" ");
                        println!("{}", transcription);

                        let mut enigo = Enigo::new(&Settings::default()).unwrap();
                        if let Err(e) = enigo.text(&transcription) {
                            eprintln!("Failed to send text: {}", e);
                            return;
                        }
                    } else {
                        eprintln!("Failed to process audio");
                    }
                }
            }
        }
    });

    if let Err(e) = hotkey_manager.start() {
        eprintln!("Failed to start hotkey manager: {}", e);
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .setup(setup_app)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
