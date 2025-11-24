use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tokio::sync::mpsc;

pub struct AudioStream {
    _stream: cpal::Stream,
}

impl AudioStream {
    pub fn setup_input(
        tx: mpsc::Sender<Vec<f32>>,
        device_index: Option<usize>,
        sample_rate: u32,
    ) -> Result<Self> {
        let host = cpal::default_host();
        
        let device = if let Some(index) = device_index {
            host.input_devices()?
                .nth(index)
                .ok_or_else(|| anyhow!("Invalid device index"))?
        } else {
            host.default_input_device()
                .ok_or_else(|| anyhow!("No input device found"))?
        };
        
        println!("Input device: {}", device.name()?);

        let mut supported_configs_range = device.supported_input_configs()?;
        let mut config = supported_configs_range
            .find(|c| c.max_sample_rate().0 >= sample_rate && c.min_sample_rate().0 <= sample_rate)
            .map(|c| c.with_sample_rate(cpal::SampleRate(sample_rate)))
            .unwrap_or_else(|| device.default_input_config().unwrap())
            .config();
        
        // Force small buffer size for low latency
        config.buffer_size = cpal::BufferSize::Fixed(512);
            
        println!("Input Config: Rate: {}, Channels: {}, Buffer: {:?}", config.sample_rate.0, config.channels, config.buffer_size);

        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                // Send data to channel
                // In a real low-latency app, we might want a ring buffer here
                // to avoid allocation, but Vec<f32> is easier for prototype.
                if let Err(e) = tx.blocking_send(data.to_vec()) {
                    eprintln!("Failed to send audio data: {}", e);
                }
            },
            move |err| eprintln!("Input stream error: {}", err),
            None,
        )?;

        stream.play()?;
        Ok(Self { _stream: stream })
    }

    pub fn setup_output(
        mut rx: mpsc::Receiver<Vec<f32>>,
        sample_rate: u32,
    ) -> Result<Self> {
        let host = cpal::default_host();
        let device = host.default_output_device()
            .ok_or_else(|| anyhow!("No output device found"))?;
            
        println!("Output device: {}", device.name()?);

        // Try to find a matching config first, otherwise fallback to default
        let mut supported_configs_range = device.supported_output_configs()?;
        let mut config = supported_configs_range
            .find(|c| c.max_sample_rate().0 >= sample_rate && c.min_sample_rate().0 <= sample_rate)
            .map(|c| c.with_sample_rate(cpal::SampleRate(sample_rate)))
            .unwrap_or_else(|| device.default_output_config().unwrap())
            .config();
        
        // Force small buffer size for low latency
        config.buffer_size = cpal::BufferSize::Fixed(512);
            
        println!("Output Config: Rate: {}, Channels: {}, Buffer: {:?}", config.sample_rate.0, config.channels, config.buffer_size);
        let _channels = config.channels as usize;

        let stream = device.build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                // Fill buffer from channel
                // This is blocking in the callback, which is not ideal for strict real-time,
                // but acceptable for this prototype. A better approach uses a lock-free ring buffer.
                
                // We need to fill 'data' completely.
                let mut written = 0;
                while written < data.len() {
                    match rx.blocking_recv() {
                        Some(samples) => {
                            let len = samples.len().min(data.len() - written);
                            // Simple copy - assuming mono/stereo match or just raw copy
                            // If channels mismatch, this will be wrong, but assuming default config matches for now
                            // or we just copy raw samples.
                            for i in 0..len {
                                data[written + i] = samples[i];
                            }
                            written += len;
                        }
                        None => {
                            // Channel closed, fill rest with silence
                            for i in written..data.len() {
                                data[i] = 0.0;
                            }
                            break;
                        }
                    }
                }
            },
            move |err| eprintln!("Output stream error: {}", err),
            None,
        )?;

        stream.play()?;
        Ok(Self { _stream: stream })
    }
}
