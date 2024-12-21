use anyhow::Result;
use log::info;
use serde::{Serialize, Deserialize};
use std::path::{Path, PathBuf};
use std::fs;
use std::marker::PhantomData;
use serde_json::Value;

const BASE_PATH: &str = ".whispr";
const SETTINGS_FILE: &str = "settings";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Model {
    pub display_name: String,
    pub url: String,
    pub filename: String,
}

#[derive(Clone)]
pub struct ConfigManager<T> where T: Serialize + for<'de> Deserialize<'de> + Default {
    config_dir: PathBuf,
    _phantom: PhantomData<T>,
}

impl<T> ConfigManager<T> where T: Serialize + for<'de> Deserialize<'de> + Default {
    pub fn new(_config_name: &str) -> Result<Self> {
        let home_dir = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
        let config_dir = home_dir.join(BASE_PATH);
        
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)?;
        }

        Ok(Self {
            config_dir,
            _phantom: PhantomData,
        })
    }

    pub fn save_config(&self, config: &T, _name: &str) -> Result<()> {
        let config_path = self.config_dir.join(format!("{}.json", SETTINGS_FILE));
        let config_str = serde_json::to_string_pretty(config)?;
        fs::write(config_path, config_str)?;
        Ok(())
    }

    pub fn load_config(&self, _name: &str) -> Result<T> {
        let config_path = self.config_dir.join(format!("{}.json", SETTINGS_FILE));
        
        if !config_path.exists() {
            let default_config = T::default();
            self.save_config(&default_config, _name)?;
            return Ok(default_config);
        }

        let config_str = fs::read_to_string(&config_path)?;
        let stored_config: Value = serde_json::from_str(&config_str)?;
        let default_config = T::default();
        let default_value = serde_json::to_value(&default_config)?;

        let (merged_value, had_missing_fields) = merge_json_values(stored_config, default_value);
        
        if had_missing_fields {
            info!("Config file had missing fields, updating with default values");
            let config: T = serde_json::from_value(merged_value.clone())?;
            self.save_config(&config, _name)?;
        }
        
        let config: T = serde_json::from_value(merged_value)?;
        Ok(config)
    }

    pub fn config_exists(&self, _name: &str) -> bool {
        self.config_dir.join(format!("{}.json", SETTINGS_FILE)).exists()
    }

    pub fn get_config_dir(&self) -> &Path {
        &self.config_dir
    }
}

fn merge_json_values(stored: Value, default: Value) -> (Value, bool) {
    match (stored, default) {
        (Value::Object(mut stored_map), Value::Object(default_map)) => {
            let mut had_missing_fields = false;
            
            for (key, default_value) in default_map {
                match stored_map.get(&key) {
                    None => {
                        info!("Missing config field: {}", key);
                        had_missing_fields = true;
                        stored_map.insert(key, default_value);
                    }
                    Some(stored_value) => {
                        if let Value::Object(_) = default_value {
                            let (merged, missing) = merge_json_values(stored_value.clone(), default_value);
                            if missing {
                                had_missing_fields = true;
                                stored_map.insert(key, merged);
                            }
                        }
                    }
                }
            }
            
            (Value::Object(stored_map), had_missing_fields)
        }
        (stored, _) => (stored, false),
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WhisprConfig {
    pub audio: AudioSettings,
    pub developer: DeveloperSettings,
    pub whisper: WhisperSettings,
    pub start_at_login: bool,
    pub keyboard_shortcut: String,
    pub model: Model,
}

impl Default for WhisprConfig {
    fn default() -> Self {
        Self {
            audio: AudioSettings::default(),
            developer: DeveloperSettings::default(),
            whisper: WhisperSettings::default(),
            start_at_login: false,
            keyboard_shortcut: "right_command_key".to_string(),
            model: Model {
                display_name: "Whisper Large v3 Turbo".to_string(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin".to_string(),
                filename: "ggml-large-v3-turbo.bin".to_string(),
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AudioSettings {
    pub device_name: Option<String>,
    pub remove_silence: bool,
    pub silence_threshold: f32,
    pub min_silence_duration: usize,
    pub recordings_dir: Option<String>,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            device_name: None,
            remove_silence: true,
            silence_threshold: 0.90,
            min_silence_duration: 250,
            recordings_dir: Some(BASE_PATH.to_string()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeveloperSettings {
    pub save_recordings: bool,
    pub whisper_logging: bool,
    pub logging: bool,
}

impl Default for DeveloperSettings {
    fn default() -> Self {
        Self {
            save_recordings: false,
            whisper_logging: false,
            logging: true, // Logging enabled by default
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WhisperSettings {
    pub model_name: String,
    pub language: Option<String>,
    pub translate: bool,
}

impl Default for WhisperSettings {
    fn default() -> Self {
        Self {
            model_name: "base.en".to_string(),
            language: None,
            translate: false,
        }
    }
}
