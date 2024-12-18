use tauri::{Manager, App, Emitter};
use std::sync::{Arc, Mutex};
use std::thread;
use std::vec::Vec;
use crate::{
    audio::AudioManager,
    window::OverlayWindow,
    hotkey::HotkeyManager,
    config::{ConfigManager, WhisprConfig},
    menu::{create_tray_menu, MenuState},
};
use whisper_rs::{WhisperContext, WhisperContextParameters, FullParams, SamplingStrategy};
use std::path::Path;

pub fn initialize_app(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle();
    let (tray_menu, _, menu_state) = create_tray_menu(&app_handle);
    
    app.manage(menu_state);
    
    let mut overlay_window = OverlayWindow::new();
    overlay_window.create_window(&app_handle);
    let overlay_window = Mutex::new(overlay_window);
    app.manage(overlay_window);
    
    let config_manager = ConfigManager::<WhisprConfig>::new("settings").expect("Failed to create config manager");
    let mut whispr_config = WhisprConfig::default();
    
    if config_manager.config_exists("settings") {
        match config_manager.load_config("settings") {
            Ok(config) => whispr_config = config,
            Err(e) => eprintln!("Failed to load configuration: {}", e),
        }
    }

    let mut audio_manager = AudioManager::new().expect("Failed to initialize audio manager");
    if let Some(device_name) = &whispr_config.audio.device_name {
        if let Err(e) = audio_manager.set_input_device(device_name) {
            eprintln!("Failed to set input device from configuration: {}", e);
        }
    }
    audio_manager.set_remove_silence(whispr_config.audio.remove_silence);

    let audio_manager = Mutex::new(audio_manager);
    app.manage(audio_manager);
    
    let model_path = Path::new("/Users/dbpprt/Downloads/ggml-large-v3-turbo.bin");
    let model_path_str = model_path.to_str().expect("Failed to convert path to string");
    let ctx = Arc::new(WhisperContext::new_with_params(
        model_path_str,
        WhisperContextParameters::default()
    ).expect("failed to load model"));

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
                app_handle_clone.emit("status-change", "Listening").unwrap();
            } else {
                audio.stop_capture();

                app_handle_clone.emit("status-change", "Transcribing").unwrap();
                
                if let Some(captured_audio) = audio.get_captured_audio(16000, 1) {
                    process_audio(captured_audio, app_handle_clone.clone(), Arc::clone(&ctx), whispr_config.clone());
                }
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

fn process_audio(captured_audio: Vec<f32>, app_handle: tauri::AppHandle, ctx: Arc<WhisperContext>, whispr_config: WhisprConfig) {
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(whispr_config.whisper.language.as_deref());
    params.set_translate(whispr_config.whisper.translate);

    let mut state = ctx.create_state().expect("failed to create state");
    state
        .full(params, &captured_audio[..])
        .expect("failed to run model");

    let num_segments = state
        .full_n_segments()
        .expect("failed to get number of segments");
    for i in 0..num_segments {
        let segment = state
            .full_get_segment_text(i)
            .expect("failed to get segment");
        let start_timestamp = state
            .full_get_segment_t0(i)
            .expect("failed to get segment start timestamp");
        let end_timestamp = state
            .full_get_segment_t1(i)
            .expect("failed to get segment end timestamp");
        println!("[{} - {}]: {}", start_timestamp, end_timestamp, segment);

        app_handle.emit("transcription-complete", segment).unwrap();
    }
}
