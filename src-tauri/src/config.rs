use anyhow::Result;
use serde::{Serialize, Deserialize};
use std::path::{Path, PathBuf};
use std::fs;
use std::marker::PhantomData;

const BASE_PATH: &str = ".whispr";
const SETTINGS_FILE: &str = "settings";

pub struct ConfigManager<T> where T: Serialize + for<'de> Deserialize<'de> {
    config_dir: PathBuf,
    _phantom: PhantomData<T>,
}

impl<T> ConfigManager<T> where T: Serialize + for<'de> Deserialize<'de> {
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
        let config_str = fs::read_to_string(config_path)?;
        let config: T = serde_json::from_str(&config_str)?;
        Ok(config)
    }

    pub fn config_exists(&self, _name: &str) -> bool {
        self.config_dir.join(format!("{}.json", SETTINGS_FILE)).exists()
    }

    pub fn get_config_dir(&self) -> &Path {
        &self.config_dir
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WhisprConfig {
    pub audio: AudioSettings,
    pub developer: DeveloperSettings,
    pub whisper: WhisperSettings,
}

impl Default for WhisprConfig {
    fn default() -> Self {
        Self {
            audio: AudioSettings::default(),
            developer: DeveloperSettings::default(),
            whisper: WhisperSettings::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
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
            silence_threshold: 0.40,
            min_silence_duration: 250,
            recordings_dir: Some(BASE_PATH.to_string()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeveloperSettings {
    pub save_recordings: bool,
}

impl Default for DeveloperSettings {
    fn default() -> Self {
        Self {
            save_recordings: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
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
