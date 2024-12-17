use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Host, Sample, SampleFormat, Stream, SizedSample};
use hound::{WavWriter, WavSpec};
use std::sync::{Arc, Mutex};
use std::fs::File;
use std::io::BufWriter;

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
}

unsafe impl Send for AudioManager {}
unsafe impl Sync for AudioManager {}

impl AudioManager {
    pub fn new() -> Result<Self> {
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
        })
    }

    pub fn set_input_device(&mut self, device_name: &str) -> Result<()> {
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

    pub fn get_current_device_name(&self) -> Result<String> {
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

    pub fn list_input_devices(&self) -> Result<Vec<String>> {
        let devices = self.host.input_devices()?;
        let mut device_names = Vec::new();
        for device in devices {
            if let Ok(name) = device.name() {
                device_names.push(name);
            }
        }
        Ok(device_names)
    }

    pub fn start_capture(&mut self) -> Result<()> {
        let config = self.input_device.default_input_config()?;
        println!("Default input config: {:?}", config);

        let spec = WavSpec {
            channels: 1,
            sample_rate: config.sample_rate().0,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let writer = WavWriter::create("debug.wav", spec)?;
        *self.wav_writer.lock().unwrap() = Some(writer);

        let is_capturing = self.is_capturing.clone();
        let wav_writer = self.wav_writer.clone();
        let silence_config = self.silence_config.clone();

        let stream = match config.sample_format() {
            SampleFormat::F32 => self.build_input_stream_f32(&config.into(), is_capturing, wav_writer, silence_config)?,
            _ => {
                let config = self.input_device.default_input_config()?.config().into();
                self.build_input_stream_f32(&config, is_capturing, wav_writer, silence_config)?
            }
        };

        stream.play()?;
        self.stream = Some(stream);
        *self.is_capturing.lock().unwrap() = true;

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
    }

    fn build_input_stream_f32(
        &self,
        config: &cpal::StreamConfig,
        is_capturing: Arc<Mutex<bool>>,
        wav_writer: Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>,
        silence_config: Arc<Mutex<SilenceConfig>>,
    ) -> Result<Stream> {
        let mut silence_counter = 0;
        let mut buffer = Vec::new();

        let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
            if !*is_capturing.lock().unwrap() {
                return;
            }

            let config = silence_config.lock().unwrap();
            if let Some(writer) = &mut *wav_writer.lock().unwrap() {
                if config.enabled {
                    buffer.clear();
                    for &sample in data {
                        let amplitude = sample.abs();
                        
                        if amplitude > config.threshold {
                            if silence_counter >= config.min_silence_duration {
                                silence_counter = 0;
                            }
                            buffer.push(sample);
                        } else {
                            silence_counter += 1;
                            if silence_counter < config.min_silence_duration {
                                buffer.push(sample);
                            }
                        }
                    }

                    for &sample in &buffer {
                        if let Err(e) = writer.write_sample(sample) {
                            eprintln!("Error writing to WAV file: {}", e);
                            return;
                        }
                    }
                } else {
                    for &sample in data {
                        if let Err(e) = writer.write_sample(sample) {
                            eprintln!("Error writing to WAV file: {}", e);
                            return;
                        }
                    }
                }
            }

            println!("Recorded {} samples", data.len());
        };

        let stream = self.input_device.build_input_stream(
            config,
            input_data_fn,
            move |err| eprintln!("An error occurred on the audio stream: {}", err),
            None,
        )?;

        Ok(stream)
    }
}

impl Drop for AudioManager {
    fn drop(&mut self) {
        self.stop_capture();
    }
}
