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

    // Initialize shared parameters
    let params = std::sync::Arc::new(std::sync::Mutex::new(voice_transformer_lib::dsp_params::DspParams::default()));
    
    // Initialize DSP with params
    let mut dsp = voice_transformer_lib::dsp::DspProcessor::new(48000, 1, params.clone())?;

    // Spawn CLI input thread
    let params_clone = params.clone();
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        let mut line = String::new();
        loop {
            line.clear();
            if stdin.read_line(&mut line).is_ok() {
                let input = line.trim();
                if input.is_empty() { continue; }
                
                let mut p = params_clone.lock().unwrap();
                let mut msg = "";
                
                match input.chars().next() {
                    Some('v') => { // Volume/Gain
                        if input.contains('+') { p.gain *= 1.1; } else { p.gain *= 0.9; }
                        msg = "Gain";
                    },
                    Some('t') => { // Threshold
                        if input.contains('+') { p.noise_threshold *= 1.5; } else { p.noise_threshold *= 0.6; }
                        msg = "Threshold";
                    },
                    Some('f') => { // Freq Shift
                        if input.contains('+') { p.freq_shift += 0.5; } else { p.freq_shift -= 0.5; }
                        msg = "Freq Shift";
                    },
                    Some('b') => { // Bandpass Toggle
                        p.filter_enabled = !p.filter_enabled;
                        msg = "Bandpass";
                    },
                    Some('?') | Some('h') => {
                        println!("\nControls: (v)+/-, (t)+/-, (f)+/-, (b)andpass toggle");
                        continue;
                    }
                    _ => {}
                }
                println!("\n[Update] {}: Gain={:.2} Thresh={:.4} Freq={:.1}Hz Filter={}", 
                    msg, p.gain, p.noise_threshold, p.freq_shift, p.filter_enabled);
            }
        }
    });

    // 2. Setup QUIC Client
    // Bind to any available port
    let client_endpoint = network::make_client_endpoint("0.0.0.0:0".parse()?, &[])?;
    
    println!("Connecting to {}...", remote_addr);
    let connection = client_endpoint.connect(remote_addr, "localhost")?.await?;
    println!("Connected!");
    println!("Controls available: v+/- (Volume), t+/- (Threshold), f+/- (Freq), b (Bandpass)");

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
        
        // Only print VU if we are not typing commands (simple heuristic: cursor at start)
        // Actually, printing \r overwrites the line. 
        // To avoid messing up input, we might want to only print if no input is pending, 
        // but that's hard with blocking stdin.
        // Let's just print. The user input will look messy but work.
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
