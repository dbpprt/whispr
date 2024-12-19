// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod hotkey;
mod window;
mod audio;
mod config;
mod menu;
mod whisper;

use std::sync::{Arc, Mutex};
use tauri::{Manager, App, Wry, Emitter};
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use enigo::{Enigo, Keyboard, Settings};
use crate::{
    audio::AudioManager,
    window::OverlayWindow,
    hotkey::HotkeyManager,
    config::{ConfigManager, WhisprConfig},
    menu::{create_tray_menu, MenuState},
    whisper::WhisperProcessor,
};

const MIN_RECORDING_DURATION: Duration = Duration::from_secs(1);

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
    recording_semaphore: Arc<Semaphore>,
    recording_start: Mutex<Option<Instant>>,
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
            recording_semaphore: Arc::new(Semaphore::new(1)),
            recording_start: Mutex::new(None),
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
    let (tray_menu, menu_state) = create_tray_menu(&app_handle);
    app.manage(menu_state);

    let handle_clone = app_handle.clone();
    let tray = tauri::tray::TrayIconBuilder::new()
        .icon(app_handle.default_window_icon().unwrap().clone())
        .menu_on_left_click(false)
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
                // Try to acquire the semaphore permit
                if let Ok(_permit) = state.recording_semaphore.try_acquire() {
                    overlay.show();
                    let mut audio = state.audio.lock().unwrap();
                    if let Err(e) = audio.start_capture() {
                        eprintln!("Failed to start audio capture: {}", e);
                        return;
                    }
                    *state.recording_start.lock().unwrap() = Some(Instant::now());
                    let _ = app_handle_clone.emit("status-change", "Listening");
                } else {
                    eprintln!("Recording already in progress");
                }
            } else {
                let mut audio = state.audio.lock().unwrap();
                audio.stop_capture();
                
                // Check recording duration
                if let Some(start_time) = state.recording_start.lock().unwrap().take() {
                    let duration = start_time.elapsed();
                    if duration < MIN_RECORDING_DURATION {
                        println!("Recording too short ({:.2}s), discarding", duration.as_secs_f32());
                        let _ = app_handle_clone.emit("status-change", "Ready");
                        overlay.hide();
                        return;
                    }
                }
                
                let _ = app_handle_clone.emit("status-change", "Transcribing");
                
                if let Some(captured_audio) = audio.get_captured_audio(16000, 1) {
                    println!("Got captured audio: {} samples", captured_audio.len());
                    
                    match state.whisper.process_audio(captured_audio) {
                        Ok(segments) => {
                            if segments.is_empty() {
                                println!("No transcription segments produced");
                                let _ = app_handle_clone.emit("status-change", "Ready");
                                overlay.hide();
                                return;
                            }
                            
                            let transcription: String = segments.iter()
                                .map(|(_, _, segment)| segment.clone())
                                .collect::<Vec<String>>()
                                .join(" ");
                            println!("Transcription: {}", transcription);

                            let mut enigo = match Enigo::new(&Settings::default()) {
                                Ok(enigo) => enigo,
                                Err(e) => {
                                    eprintln!("Failed to create Enigo instance: {}", e);
                                    let _ = app_handle_clone.emit("status-change", "Ready");
                                    overlay.hide();
                                    return;
                                }
                            };
                            
                            if let Err(e) = enigo.text(&transcription) {
                                eprintln!("Failed to send text: {}", e);
                                let _ = app_handle_clone.emit("status-change", "Ready");
                                overlay.hide();
                                return;
                            }
                            
                            let _ = app_handle_clone.emit("status-change", "Ready");
                        }
                        Err(e) => {
                            eprintln!("Failed to process audio: {}", e);
                            let _ = app_handle_clone.emit("status-change", "Ready");
                            overlay.hide();
                            return;
                        }
                    }
                } else {
                    println!("No audio captured");
                    let _ = app_handle_clone.emit("status-change", "Ready");
                    overlay.hide();
                    return;
                }
                
                overlay.hide();
                
                // Release the semaphore permit
                state.recording_semaphore.add_permits(1);
            }
        }
    }, whispr_config.clone());

    if let Err(e) = hotkey_manager.start() {
        eprintln!("Failed to start hotkey manager: {}", e);
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            println!("{}, {argv:?}, {cwd}", app.package_info().name);
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())  // Register the process plugin
        .setup(setup_app)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
