use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use log::{error, warn, info, debug};
use cpal::{Device, Host, Stream, StreamConfig};
use hound::{WavWriter, WavSpec};
use std::sync::{Arc, Mutex};
use std::fs::File;
use std::io::BufWriter;
use crate::config::{ConfigManager, WhisprConfig};
use chrono::Local;
use anyhow::Error;
use std::collections::VecDeque;
use samplerate::{convert, ConverterType};
use std::time::Instant;

fn audio_resample(data: &[f32], sample_rate0: u32, sample_rate: u32, channels: u16) -> Vec<f32> {
    convert(
        sample_rate0 as _,
        sample_rate as _,
        channels as _,
        ConverterType::SincBestQuality,
        data,
    ).unwrap_or_default()
}

fn stereo_to_mono(stereo_data: &[f32]) -> Vec<f32> {
    let mut mono_data = Vec::with_capacity(stereo_data.len() / 2);
    for chunk in stereo_data.chunks_exact(2) {
        let average = (chunk[0] + chunk[1]) / 2.0;
        mono_data.push(average);
    }
    mono_data
}

#[derive(Clone)]
pub struct SilenceConfig {
    enabled: bool,
    threshold: f32,
    min_silence_duration: usize,
}

impl Default for SilenceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: 0.01,
            min_silence_duration: 1000,
        }
    }
}

pub struct AudioManager {
    host: Host,
    input_device: Device,
    stream: Option<Stream>,
    is_capturing: Arc<Mutex<bool>>,
    wav_writer: Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>,
    silence_config: Arc<Mutex<SilenceConfig>>,
    _start_time: Arc<Mutex<Option<Instant>>>,
    captured_audio: Arc<Mutex<VecDeque<f32>>>,
}

unsafe impl Send for AudioManager {}
unsafe impl Sync for AudioManager {}

impl AudioManager {
    pub fn new() -> Result<Self, Error> {
        let host = cpal::default_host();
        let input_device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("No input device available"))?;
        
        info!("Using input device: {}", input_device.name()?);

        Ok(Self {
            host,
            input_device,
            stream: None,
            is_capturing: Arc::new(Mutex::new(false)),
            wav_writer: Arc::new(Mutex::new(None)),
            silence_config: Arc::new(Mutex::new(SilenceConfig::default())),
            _start_time: Arc::new(Mutex::new(None)),
            captured_audio: Arc::new(Mutex::new(VecDeque::new())),
        })
    }

    pub fn set_input_device(&mut self, device_name: &str) -> Result<(), Error> {
        let devices = self.host.input_devices()?;
        for device in devices {
            if let Ok(name) = device.name() {
                if name == device_name {
                    self.input_device = device;
                    return Ok(());
                }
            }
        }
        Err(anyhow::anyhow!("Device not found: {}", device_name))
    }

    pub fn get_current_device_name(&self) -> Result<String, Error> {
        Ok(self.input_device.name()?)
    }

    pub fn configure_silence_removal(&self, enabled: bool, threshold: Option<f32>, min_silence_duration: Option<usize>) {
        let mut config = self.silence_config.lock().unwrap();
        config.enabled = enabled;
        if let Some(t) = threshold {
            config.threshold = t;
        }
        if let Some(d) = min_silence_duration {
            config.min_silence_duration = d;
        }
    }

    pub fn is_silence_removal_enabled(&self) -> bool {
        self.silence_config.lock().unwrap().enabled
    }

    pub fn list_input_devices(&self) -> Result<Vec<String>, Error> {
        let devices = self.host.input_devices()?;
        let mut device_names = Vec::new();
        for device in devices {
            if let Ok(name) = device.name() {
                device_names.push(name);
            }
        }
        Ok(device_names)
    }

    pub fn start_capture(&mut self) -> Result<(), Error> {
        let default_config = self.input_device.default_input_config()?;
        debug!("Default input config: {:?}", default_config);

        let config = StreamConfig {
            channels: default_config.channels(),
            sample_rate: default_config.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };
        debug!("Using input config: {:?}", config);

        let spec = WavSpec {
            channels: config.channels,
            sample_rate: config.sample_rate.0,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };

        let config_manager = ConfigManager::<WhisprConfig>::new("settings").expect("Failed to create config manager");
        let whispr_config = config_manager.load_config("settings").expect("Failed to load configuration");

        let writer = if whispr_config.developer.save_recordings {
            let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
            let recordings_dir = config_manager.get_config_dir().join("recordings");
            let file_path = recordings_dir.join(format!("{}.wav", timestamp));
            std::fs::create_dir_all(&recordings_dir).expect("Failed to create recordings directory");
            info!("Saving recording to: {}", file_path.display());
            Some(WavWriter::create(file_path, spec)?)
        } else {
            None
        };

        *self.wav_writer.lock().unwrap() = writer;
        *self._start_time.lock().unwrap() = Some(Instant::now());

        let is_capturing = self.is_capturing.clone();
        let wav_writer = self.wav_writer.clone();
        let silence_config = self.silence_config.clone();
        let _start_time = self._start_time.clone();
        let captured_audio = self.captured_audio.clone();

        let stream = self.build_input_stream_f32(&config, is_capturing, wav_writer, silence_config, _start_time, captured_audio)?;

        stream.play()?;
        self.stream = Some(stream);
        *self.is_capturing.lock().unwrap() = true;

        info!("Capture started");

        Ok(())
    }

    pub fn stop_capture(&mut self) {
        // First mark as not capturing to prevent any new data from being processed
        *self.is_capturing.lock().unwrap() = false;

        // Ensure proper stream shutdown
        if let Some(stream) = self.stream.take() {
            // Pause the stream before dropping to ensure clean shutdown
            if let Err(e) = stream.pause() {
                error!("Error pausing stream: {}", e);
            }
            drop(stream);
        }
        
        // Clean up WAV writer
        if let Some(writer) = self.wav_writer.lock().unwrap().take() {
            if let Err(e) = writer.finalize() {
                error!("Error finalizing WAV file: {}", e);
            }
        }

        // Log timing information
        if let Some(start_time) = self._start_time.lock().unwrap().take() {
            let duration = start_time.elapsed();
            info!("Recording stopped after: {:.2}s", duration.as_secs_f32());
        }
        
        // Small delay to ensure all audio data has been processed
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Log audio buffer size but don't clear it yet - it will be cleared when get_captured_audio is called
        let samples = self.captured_audio.lock().unwrap().len();
        debug!("Audio buffer contains {} samples", samples);

        // Additional delay to ensure complete cleanup
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    fn build_input_stream_f32(
        &self,
        config: &StreamConfig,
        is_capturing: Arc<Mutex<bool>>,
        wav_writer: Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>,
        silence_config: Arc<Mutex<SilenceConfig>>,
        _start_time: Arc<Mutex<Option<Instant>>>,
        captured_audio: Arc<Mutex<VecDeque<f32>>>,
    ) -> Result<Stream, Error> {
        // Clear any existing audio data before starting new capture
        captured_audio.lock().unwrap().clear();

        let mut silence_counter = 0usize;
        let mut is_in_silence = false;

        let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
            if !*is_capturing.lock().unwrap() {
                return;
            }

            // Get all silence config values in one lock
            let silence_cfg = {
                let cfg = silence_config.lock().unwrap();
                (cfg.enabled, cfg.threshold, cfg.min_silence_duration)
            };
            let (is_silence_enabled, silence_threshold, min_silence_duration) = silence_cfg;

            // Process samples without holding locks
            let mut samples_to_keep = Vec::with_capacity(data.len());
            
            if is_silence_enabled {
                for &sample in data {
                    let amplitude = sample.abs();
                    if amplitude > silence_threshold {
                        if is_in_silence {
                            silence_counter = 0;
                            is_in_silence = false;
                        }
                        samples_to_keep.push(sample);
                    } else if !is_in_silence {
                        silence_counter += 1;
                        if silence_counter >= min_silence_duration {
                            is_in_silence = true;
                        } else {
                            samples_to_keep.push(sample);
                        }
                    }
                }
            } else {
                samples_to_keep.extend_from_slice(data);
            }

            // Write samples in a single batch with minimal lock time
            {
                let mut writer_guard = wav_writer.lock().unwrap();
                if let Some(ref mut writer) = *writer_guard {
                    // Write all samples at once to minimize lock time
                    for &sample in &samples_to_keep {
                        writer.write_sample(sample).unwrap_or_else(|e| error!("Error writing sample: {}", e));
                    }
                }
            } // writer lock is released here

            // Update audio buffer in a single batch with minimal lock time
            {
                let mut audio_buffer = captured_audio.lock().unwrap();
                audio_buffer.extend(samples_to_keep);
            } // audio buffer lock is released here
        };

        let stream = self.input_device.build_input_stream(
            config,
            input_data_fn,
            move |err| error!("An error occurred on the audio stream: {}", err),
            None,
        )?;

        Ok(stream)
    }

    pub fn set_remove_silence(&mut self, remove_silence: bool) {
        self.configure_silence_removal(remove_silence, None, None);
    }

    pub fn get_captured_audio(&self, desired_sample_rate: u32, desired_channels: u16) -> Option<Vec<f32>> {
        let mut audio_buffer = self.captured_audio.lock().unwrap();
        if audio_buffer.is_empty() {
            debug!("Audio buffer is empty");
            None
        } else {
            let buffer_len = audio_buffer.len();
            debug!("Processing {} samples from audio buffer", buffer_len);
            
            let audio_data: Vec<f32> = Vec::from_iter(audio_buffer.drain(..));
            let config = match self.input_device.default_input_config() {
                Ok(cfg) => cfg,
                Err(e) => {
                    error!("Failed to get input config: {}", e);
                    return None;
                }
            };
            
            let captured_sample_rate = config.sample_rate().0;
            let captured_channels = config.channels();
            debug!("Captured format: {}Hz, {} channels", captured_sample_rate, captured_channels);
            debug!("Desired format: {}Hz, {} channels", desired_sample_rate, desired_channels);

            let mut processed_audio = audio_data;
            let initial_len = processed_audio.len();

            // Only convert stereo to mono if we have stereo input and want mono output
            if captured_channels == 2 && desired_channels == 1 {
                processed_audio = stereo_to_mono(&processed_audio);
                debug!("Converted stereo to mono: {} -> {} samples", initial_len, processed_audio.len());
            } else if captured_channels > 2 {
                // Handle other multi-channel formats (if any) by averaging all channels
                let samples_per_frame = captured_channels as usize;
                let mut mono_data = Vec::with_capacity(processed_audio.len() / samples_per_frame);
                for chunk in processed_audio.chunks_exact(samples_per_frame) {
                    let average = chunk.iter().sum::<f32>() / samples_per_frame as f32;
                    mono_data.push(average);
                }
                processed_audio = mono_data;
                debug!("Converted multi-channel to mono: {} -> {} samples", initial_len, processed_audio.len());
            }

            // Resample if needed
            if captured_sample_rate != desired_sample_rate {
                let before_resample = processed_audio.len();
                processed_audio = audio_resample(
                    &processed_audio,
                    captured_sample_rate,
                    desired_sample_rate,
                    desired_channels,
                );
                debug!("Resampled audio: {} -> {} samples", before_resample, processed_audio.len());
            }

            if processed_audio.is_empty() {
                warn!("Processed audio is empty after conversion");
                None
            } else {
                debug!("Successfully processed {} samples", processed_audio.len());
                Some(processed_audio)
            }
        }
    }
}

impl Drop for AudioManager {
    fn drop(&mut self) {
        self.stop_capture();
    }
}
