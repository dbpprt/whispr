use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Host, Sample, SampleFormat, Stream, SizedSample};
use hound::{WavWriter, WavSpec};
use std::sync::{Arc, Mutex};
use std::fs::File;
use std::io::BufWriter;

pub struct AudioManager {
    host: Host,
    input_device: Device,
    stream: Option<Stream>,
    is_capturing: Arc<Mutex<bool>>,
    wav_writer: Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>,
}

// Required for thread safety
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
        })
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

        // Create WAV writer
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

        // Convert all input to f32 format
        let stream = match config.sample_format() {
            SampleFormat::F32 => self.build_input_stream_f32(&config.into(), is_capturing, wav_writer)?,
            _ => {
                let config = self.input_device.default_input_config()?.config().into();
                self.build_input_stream_f32(&config, is_capturing, wav_writer)?
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
        
        // Finalize WAV file
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
    ) -> Result<Stream> {
        let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
            if !*is_capturing.lock().unwrap() {
                return;
            }

            if let Some(writer) = &mut *wav_writer.lock().unwrap() {
                // Write samples directly to WAV file
                for &sample in data {
                    if let Err(e) = writer.write_sample(sample) {
                        eprintln!("Error writing to WAV file: {}", e);
                        return;
                    }
                }
            }

            // Print debug info
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
