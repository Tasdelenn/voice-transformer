use anyhow::Result;
use clap::Parser;
use tokio::sync::mpsc;
use voice_transformer_lib::{audio, network};
use std::net::SocketAddr;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Receiver IP address
    #[arg(long, default_value = "127.0.0.1")]
    ip: String,

    /// Receiver port
    #[arg(long, default_value = "5000")]
    port: u16,

    /// List available input devices
    #[arg(long)]
    list_devices: bool,

    /// Input device index
    #[arg(long)]
    device: Option<usize>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // List devices if requested
    if args.list_devices {
        use cpal::traits::HostTrait;
        let host = cpal::default_host();
        println!("Available Input Devices:");
        match host.input_devices() {
            Ok(devices) => {
                for (i, device) in devices.enumerate() {
                    use cpal::traits::DeviceTrait;
                    println!("{}: {}", i, device.name().unwrap_or("Unknown".to_string()));
                }
            }
            Err(e) => eprintln!("Error listing devices: {}", e),
        }
        return Ok(());
    }

    let remote_addr: SocketAddr = format!("{}:{}", args.ip, args.port).parse()?;
    
    println!("Starting Sender connecting to {}", remote_addr);

    // 1. Setup Audio Input
    let (tx, mut rx) = mpsc::channel::<Vec<f32>>(100); // Buffer up to 100 chunks
    // Pass device index if specified
    let _audio_stream = audio::AudioStream::setup_input(tx, args.device)?;
    println!("Audio input started");

    // Initialize DSP
    let mut dsp = voice_transformer_lib::dsp::DspProcessor::new(48000, 1)?;

    // 2. Setup QUIC Client
    // Bind to any available port
    let client_endpoint = network::make_client_endpoint("0.0.0.0:0".parse()?, &[])?;
    
    println!("Connecting to {}...", remote_addr);
    let connection = client_endpoint.connect(remote_addr, "localhost")?.await?;
    println!("Connected!");

    // 3. Open stream
    let mut send_stream = connection.open_uni().await?;
    println!("Stream opened");

    // 4. Streaming Loop
    while let Some(mut samples) = rx.recv().await {
        // Process audio with DSP
        if let Err(e) = dsp.process_frame(&mut samples) {
            eprintln!("DSP error: {}", e);
        }

        // Calculate RMS for debug
        let rms: f32 = (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt();
        
        // VU Meter (32 chars)
        // RMS is usually low (0.0 - 0.5), so we boost it for visualization
        let display_rms = (rms * 5.0).clamp(0.0, 1.0); 
        let bar_width = 32;
        let filled = (display_rms * bar_width as f32) as usize;
        let empty = bar_width - filled;
        
        let bar = format!("[{}{}]", "█".repeat(filled), " ".repeat(empty));
        print!("\rLevel: {} {:.4}   ", bar, rms);
        use std::io::Write;
        std::io::stdout().flush()?;

        // Convert f32 samples to bytes
        let mut bytes = Vec::with_capacity(samples.len() * 4);
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }

        // Write to stream
        send_stream.write_all(&bytes).await?;
    }

    Ok(())
}
