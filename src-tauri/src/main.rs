// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod hotkey;
mod window;
mod audio;
mod config;
mod menu;
mod whisper;
mod logging;

use log::{error, warn, info, debug};
use std::sync::{Arc, Mutex};
use tauri::{image::Image, path::BaseDirectory, App, Emitter, Manager, Wry};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use enigo::{Enigo, Keyboard, Settings};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_shell::ShellExt;

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
            .map_err(|e| WhisprError::ConfigError(e.to_string()))?;
        
        let model_path = dirs::home_dir()
            .ok_or_else(|| WhisprError::SystemError("Could not find home directory".to_string()))?
            .join(".whispr")
            .join("model.bin");
        let whisper = WhisperProcessor::new(&model_path, config)
            .map_err(WhisprError::WhisperError)?;
     
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
    
    // Check if model file exists
    let model_path = config_manager.get_config_dir().join("model.bin");
    if !model_path.exists() {
        app.dialog()
            .message("Model file not found at ~/.whispr/model.bin - see README.md")
            .kind(MessageDialogKind::Error)
            .title("Error")
            .blocking_show();
        
        let _ = app.shell().command("open")
            .args(["https://github.com/dbpprt/whispr?tab=readme-ov-file#usage"])
            .spawn();

        app.handle().exit(1);
        return Ok(());
    }
    
    let mut whispr_config = if config_manager.config_exists("settings") {
        config_manager.load_config("settings")
            .map_err(|e| WhisprError::ConfigError(e.to_string()))?
    } else {
        WhisprConfig::default()
    };

    // Set default audio device if none is configured
    if whispr_config.audio.device_name.is_none() {
        let temp_audio = AudioManager::new()
            .map_err(|e| WhisprError::AudioError(e.to_string()))?;
        if let Some(first_device) = temp_audio.list_input_devices()
            .map_err(|e| WhisprError::AudioError(e.to_string()))?
            .first() {
            whispr_config.audio.device_name = Some(first_device.clone());
            config_manager.save_config(&whispr_config, "settings")
                .map_err(|e| WhisprError::ConfigError(e.to_string()))?;
        }
    }

    // Initialize Enigo once to prompt for permissions
    match Enigo::new(&Settings::default()) {
        Ok(_) => info!("Successfully initialized Enigo"),
        Err(e) => warn!("Failed to initialize Enigo: {}", e),
    }

    // Initialize application state
    let state = AppState::new(whispr_config.clone())?;
    state.configure_audio(&whispr_config)?;
    
    // Create window
    state.overlay.lock().unwrap().create_window(app_handle);
    
    // Store state
    app.manage(state);

    // Setup tray and menu
    let (tray_menu, menu_state) = create_tray_menu(app_handle);
    app.manage(menu_state);

    // this should be next only
    let icon_resource_path = app.path().resolve("icons/tray/tray_128.png", BaseDirectory::Resource)?;
    let handle_clone = app.handle().clone();

    let tray = tauri::tray::TrayIconBuilder::new()
        .icon(Image::from_path(icon_resource_path)?) // <- this is next
        .menu_on_left_click(false)
        .menu(&tray_menu)
        .on_menu_event(move |app, event| {
            let menu_state = handle_clone.state::<MenuState<_>>();
            crate::menu::handle_menu_event(app.clone(), &event.id().0, &menu_state);
        })
        .build(app.handle())
        .map_err(|e| Box::new(WhisprError::SystemError(e.to_string())) as Box<dyn std::error::Error>)?;

    app.manage(tray);

    // Setup hotkey manager
    let app_handle_clone = app.handle().clone();
    let mut hotkey_manager = HotkeyManager::new(move |is_speaking| {
        if let Some(state) = app_handle_clone.try_state::<AppState>() {
            let overlay = state.overlay.lock().unwrap();
            
            if is_speaking {
                // Try to acquire the semaphore permit
                if let Ok(_permit) = state.recording_semaphore.try_acquire() {
                    overlay.show();
                    let mut audio = state.audio.lock().unwrap();
                    if let Err(e) = audio.start_capture() {
                        error!("Failed to start audio capture: {}", e);
                        return;
                    }
                    *state.recording_start.lock().unwrap() = Some(Instant::now());
                    let _ = app_handle_clone.emit("status-change", "Listening");
                } else {
                    warn!("Recording already in progress");
                }
            } else {
                let mut audio = state.audio.lock().unwrap();
                audio.stop_capture();
                
                // Check recording duration
                if let Some(start_time) = state.recording_start.lock().unwrap().take() {
                    let duration = start_time.elapsed();
                    if duration < MIN_RECORDING_DURATION {
                        debug!("Recording too short ({:.2}s), discarding", duration.as_secs_f32());
                        let _ = app_handle_clone.emit("status-change", "Ready");
                        overlay.hide();
                        return;
                    }
                }
                
                let _ = app_handle_clone.emit("status-change", "Transcribing");
                
                if let Some(captured_audio) = audio.get_captured_audio(16000, 1) {
                    debug!("Got captured audio: {} samples", captured_audio.len());
                    
                    match state.whisper.process_audio(captured_audio) {
                        Ok(segments) => {
                            if segments.is_empty() {
                                info!("No transcription segments produced");
                                let _ = app_handle_clone.emit("status-change", "Ready");
                                overlay.hide();
                                return;
                            }
                            
                            let transcription: String = segments.iter()
                                .map(|(_, _, segment)| segment.clone())
                                .collect::<Vec<String>>()
                                .join(" ");
                            info!("Transcription: {}", transcription);

                            // Create a new Enigo instance for text input
                            let mut enigo = match Enigo::new(&Settings::default()) {
                                Ok(enigo) => enigo,
                                Err(e) => {
                                    error!("Failed to create Enigo instance: {}", e);
                                    let _ = app_handle_clone.emit("status-change", "Ready");
                                    overlay.hide();
                                    return;
                                }
                            };
                            
                            if let Err(e) = enigo.text(&transcription) {
                                error!("Failed to send text: {}", e);
                                let _ = app_handle_clone.emit("status-change", "Ready");
                                overlay.hide();
                                return;
                            }
                            
                            let _ = app_handle_clone.emit("status-change", "Ready");
                        }
                        Err(e) => {
                            error!("Failed to process audio: {}", e);
                            let _ = app_handle_clone.emit("status-change", "Ready");
                            overlay.hide();
                            return;
                        }
                    }
                } else {
                    info!("No audio captured");
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
        error!("Failed to start hotkey manager: {}", e);
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
fn main() {
    if let Err(e) = logging::setup_logging() {
        eprintln!("Failed to initialize logging: {}", e);
    }
    
    info!("Starting Whispr application");
    
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            info!("{}, {argv:?}, {cwd}", app.package_info().name);
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())  // Register the process plugin
        .setup(setup_app)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
