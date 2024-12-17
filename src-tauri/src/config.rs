use anyhow::Result;
use serde::{Serialize, Deserialize};
use std::path::{Path, PathBuf};
use std::fs;
use std::marker::PhantomData;

pub struct ConfigManager<T> where T: Serialize + for<'de> Deserialize<'de> {
    config_dir: PathBuf,
    _phantom: PhantomData<T>,
}

impl<T> ConfigManager<T> where T: Serialize + for<'de> Deserialize<'de> {
    pub fn new(config_name: &str) -> Result<Self> {
        let home_dir = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
        let config_dir = home_dir.join(".whispr");
        
        // Create .whispr directory if it doesn't exist
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)?;
        }

        Ok(Self {
            config_dir,
            _phantom: PhantomData,
        })
    }

    pub fn save_config(&self, config: &T, name: &str) -> Result<()> {
        let config_path = self.config_dir.join(format!("{}.json", name));
        let config_str = serde_json::to_string_pretty(config)?;
        fs::write(config_path, config_str)?;
        Ok(())
    }

    pub fn load_config(&self, name: &str) -> Result<T> {
        let config_path = self.config_dir.join(format!("{}.json", name));
        let config_str = fs::read_to_string(config_path)?;
        let config = serde_json::from_str(&config_str)?;
        Ok(config)
    }

    pub fn save_artifact(&self, artifact: &[u8], name: &str) -> Result<()> {
        let artifacts_dir = self.config_dir.join("artifacts");
        if !artifacts_dir.exists() {
            fs::create_dir_all(&artifacts_dir)?;
        }

        let artifact_path = artifacts_dir.join(name);
        fs::write(artifact_path, artifact)?;
        Ok(())
    }

    pub fn load_artifact(&self, name: &str) -> Result<Vec<u8>> {
        let artifact_path = self.config_dir.join("artifacts").join(name);
        let artifact = fs::read(artifact_path)?;
        Ok(artifact)
    }

    pub fn artifact_exists(&self, name: &str) -> bool {
        self.config_dir.join("artifacts").join(name).exists()
    }

    pub fn config_exists(&self, name: &str) -> bool {
        self.config_dir.join(format!("{}.json", name)).exists()
    }

    pub fn get_config_dir(&self) -> &Path {
        &self.config_dir
    }

    pub fn list_configs(&self) -> Result<Vec<String>> {
        let mut configs = Vec::new();
        for entry in fs::read_dir(&self.config_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
                if let Some(name) = path.file_stem() {
                    if let Some(name_str) = name.to_str() {
                        configs.push(name_str.to_string());
                    }
                }
            }
        }
        Ok(configs)
    }

    pub fn list_artifacts(&self) -> Result<Vec<String>> {
        let artifacts_dir = self.config_dir.join("artifacts");
        if !artifacts_dir.exists() {
            return Ok(Vec::new());
        }

        let mut artifacts = Vec::new();
        for entry in fs::read_dir(artifacts_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name() {
                    if let Some(name_str) = name.to_str() {
                        artifacts.push(name_str.to_string());
                    }
                }
            }
        }
        Ok(artifacts)
    }
}

// Example configuration structs
#[derive(Debug, Serialize, Deserialize)]
pub struct AudioConfig {
    pub device_name: Option<String>,
    pub remove_silence: bool,
    pub silence_threshold: f32,
    pub min_silence_duration: usize,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            device_name: None,
            remove_silence: false,
            silence_threshold: 0.01,
            min_silence_duration: 1000,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WhisperConfig {
    pub model_name: String,
    pub language: Option<String>,
    pub translate: bool,
}

impl Default for WhisperConfig {
    fn default() -> Self {
        Self {
            model_name: "base.en".to_string(),
            language: None,
            translate: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::env;

    #[test]
    fn test_config_manager() -> Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempdir()?;
        env::set_var("HOME", temp_dir.path());

        // Test audio config
        let audio_config = AudioConfig::default();
        let config_manager = ConfigManager::<AudioConfig>::new("audio")?;
        
        // Test saving and loading config
        config_manager.save_config(&audio_config, "audio")?;
        let loaded_config = config_manager.load_config("audio")?;
        assert_eq!(loaded_config.remove_silence, audio_config.remove_silence);
        
        // Test artifacts
        let test_data = b"test artifact data";
        config_manager.save_artifact(test_data, "test.bin")?;
        let loaded_data = config_manager.load_artifact("test.bin")?;
        assert_eq!(&loaded_data, test_data);

        Ok(())
    }
}
