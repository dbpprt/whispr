use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
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
        
        println!("Using input device: {}", input_device.name()?);

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
        println!("Default input config: {:?}", default_config);

        let config = StreamConfig {
            channels: default_config.channels(),
            sample_rate: default_config.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };
        println!("Using input config: {:?}", config);

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
            println!("Saving recording to: {}", file_path.display());
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

        println!("Capture started");

        Ok(())
    }

    pub fn stop_capture(&mut self) {
        self.stream = None;
        *self.is_capturing.lock().unwrap() = false;
        
        if let Some(writer) = self.wav_writer.lock().unwrap().take() {
            if let Err(e) = writer.finalize() {
                eprintln!("Error finalizing WAV file: {}", e);
            }
        }

        if let Some(start_time) = self._start_time.lock().unwrap().take() {
            let duration = start_time.elapsed();
            println!("Recording stopped after: {:.2}s", duration.as_secs_f32());
        }
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
        let mut silence_counter = 0;
        let mut is_in_silence = false;

        let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
            if !*is_capturing.lock().unwrap() {
                return;
            }

            let config = silence_config.lock().unwrap();
            
            if let Some(ref mut writer) = *wav_writer.lock().unwrap() {
                if config.enabled {
                    for &sample in data {
                        let amplitude = sample.abs();
                        
                        if amplitude > config.threshold {
                            if is_in_silence {
                                is_in_silence = false;
                                silence_counter = 0;
                            }
                            writer.write_sample(sample).unwrap_or_else(|e| eprintln!("Error writing sample: {}", e));
                            captured_audio.lock().unwrap().push_back(sample);
                        } else {
                            if !is_in_silence {
                                silence_counter += 1;
                                if silence_counter >= config.min_silence_duration {
                                    is_in_silence = true;
                                } else {
                                    writer.write_sample(sample).unwrap_or_else(|e| eprintln!("Error writing sample: {}", e));
                                    captured_audio.lock().unwrap().push_back(sample);
                                }
                            }
                        }
                    }
                } else {
                    for &sample in data {
                        writer.write_sample(sample).unwrap_or_else(|e| eprintln!("Error writing sample: {}", e));
                        captured_audio.lock().unwrap().push_back(sample);
                    }
                }
            }
        };

        let stream = self.input_device.build_input_stream(
            config,
            input_data_fn,
            move |err| eprintln!("An error occurred on the audio stream: {}", err),
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
            None
        } else {
            let audio_data: Vec<f32> = Vec::from_iter(audio_buffer.drain(..));
            let config = self.input_device.default_input_config().ok()?;
            let captured_sample_rate = config.sample_rate().0;
            let captured_channels = config.channels();

            let mut processed_audio = audio_data;

            // Only convert stereo to mono if we have stereo input and want mono output
            if captured_channels == 2 && desired_channels == 1 {
                processed_audio = stereo_to_mono(&processed_audio);
            } else if captured_channels > 2 {
                // Handle other multi-channel formats (if any) by averaging all channels
                let samples_per_frame = captured_channels as usize;
                let mut mono_data = Vec::with_capacity(processed_audio.len() / samples_per_frame);
                for chunk in processed_audio.chunks_exact(samples_per_frame) {
                    let average = chunk.iter().sum::<f32>() / samples_per_frame as f32;
                    mono_data.push(average);
                }
                processed_audio = mono_data;
            }
            // Note: If input is already mono, no channel conversion needed

            // Resample if needed
            if captured_sample_rate != desired_sample_rate {
                processed_audio = audio_resample(
                    &processed_audio,
                    captured_sample_rate,
                    desired_sample_rate,
                    desired_channels,
                );
            }

            Some(processed_audio)
        }
    }
}

impl Drop for AudioManager {
    fn drop(&mut self) {
        self.stop_capture();
    }
}
