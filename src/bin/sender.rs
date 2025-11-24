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

    /// Sample rate (default: 44100)
    #[arg(long, default_value = "44100")]
    sample_rate: u32,

    /// Buffer size in samples (default: 512 for local, try 2048 for network)
    #[arg(long, default_value = "512")]
    buffer_size: u32,
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
    let (_audio_stream, actual_sample_rate) = audio::AudioStream::setup_input(tx, args.device, args.sample_rate, args.buffer_size)?;
    println!("Audio input started at {} Hz (requested: {})", actual_sample_rate, args.sample_rate);

    // Initialize shared parameters
    let params = std::sync::Arc::new(std::sync::Mutex::new(voice_transformer_lib::dsp_params::DspParams::default()));
    
    // Initialize shared audio buffer for visualization
    let audio_buffer = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

    // Initialize UI App
    let app = std::sync::Arc::new(std::sync::Mutex::new(voice_transformer_lib::ui::App::new(params.clone(), audio_buffer.clone())));

    // Initialize DSP with params (use ACTUAL hardware sample rate, not requested)
    let mut dsp = voice_transformer_lib::dsp::DspProcessor::new(actual_sample_rate, 1, params.clone())?;

    // 2. Setup QUIC Client
    // Bind to any available port
    let client_endpoint = network::make_client_endpoint("0.0.0.0:0".parse()?, &[])?;
    
    // Connect in background or wait? 
    // We need to wait for connection to start streaming.
    // Let's do connection first, then start TUI.
    println!("Connecting to {}...", remote_addr);
    let connection = client_endpoint.connect(remote_addr, "localhost")?.await?;
    println!("Connected! Starting TUI...");

    // 3. Open stream
    let mut send_stream = connection.open_uni().await?;
    
    // Spawn Audio Processing & Streaming Task
    let audio_buffer_clone = audio_buffer.clone();
    tokio::spawn(async move {
        while let Some(mut samples) = rx.recv().await {
            // Process audio with DSP
            if let Err(e) = dsp.process_frame(&mut samples) {
                // Log to stderr (might mess up TUI, but acceptable for fatal errors)
                // Ideally log to a file or UI status line
            }

            // Update visualization buffer
            {
                let mut buf = audio_buffer_clone.lock().unwrap();
                // Append new samples
                buf.extend_from_slice(&samples);
                // Keep only last N samples (e.g. 2048) to prevent memory growth
                if buf.len() > 2048 {
                    let excess = buf.len() - 2048;
                    buf.drain(0..excess);
                }
            }

            // Convert f32 samples to bytes
            let mut bytes = Vec::with_capacity(samples.len() * 4);
            for sample in samples {
                bytes.extend_from_slice(&sample.to_le_bytes());
            }

            // Write to stream
            if let Err(_) = send_stream.write_all(&bytes).await {
                break; // Stop if connection closed
            }
        }
    });

    // Run TUI (Blocking)
    // We use spawn_blocking to allow other tokio tasks to run smoothly if we were doing more,
    // but running it directly here is also fine as long as we don't block the runtime.
    // Since we are in #[tokio::main], main is an async task. Blocking it blocks one worker.
    // It's better to use spawn_blocking for the TUI loop.
    let app_clone = app.clone();
    tokio::task::spawn_blocking(move || {
        voice_transformer_lib::ui::run_tui(app_clone)
    }).await??;

    Ok(())
}
