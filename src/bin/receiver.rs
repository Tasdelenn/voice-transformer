use anyhow::Result;
use clap::Parser;
use tokio::sync::mpsc;
use voice_transformer_lib::{audio, network, ui, dsp_params::DspParams};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Listen port
    #[arg(long, default_value = "5000")]
    port: u16,

    /// Sample rate (default: 44100)
    #[arg(long, default_value = "44100")]
    sample_rate: u32,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let addr: SocketAddr = format!("0.0.0.0:{}", args.port).parse()?;
    
    println!("Starting Receiver listening on {}", addr);

    // Shared state for TUI
    let params = Arc::new(Mutex::new(DspParams::default()));
    let audio_buffer = Arc::new(Mutex::new(Vec::new()));
    let app = Arc::new(Mutex::new(ui::App::new(params.clone(), audio_buffer.clone())));

    // 1. Setup Audio Output
    let (tx, rx) = mpsc::channel::<Vec<f32>>(100);
    // We are not using DSP on receiver yet, so params are just for UI
    let _audio_stream = audio::AudioStream::setup_output(rx, args.sample_rate)?;
    println!("Audio output ready at {} Hz", args.sample_rate);

    // 2. Setup QUIC Server
    let (endpoint, _cert) = network::make_server_endpoint(addr)?;
    
    // 3. Spawn TUI in a blocking task
    let app_clone = app.clone();
    std::thread::spawn(move || {
        if let Err(e) = ui::run_tui(app_clone) {
            eprintln!("TUI Error: {}", e);
        }
        std::process::exit(0);
    });

    println!("Waiting for connections...");

    // 4. Accept Loop
    while let Some(connecting) = endpoint.accept().await {
        // println!("New connection incoming..."); // TUI might overwrite this
        let tx = tx.clone();
        let audio_buffer = audio_buffer.clone();
        
        tokio::spawn(async move {
            let connection = match connecting.await {
                Ok(conn) => conn,
                Err(e) => {
                    eprintln!("Connection failed: {}", e);
                    return;
                }
            };
            // println!("Connection established from {}", connection.remote_address());

            while let Ok(mut recv_stream) = connection.accept_uni().await {
                // println!("Stream accepted");
                let tx = tx.clone();
                let audio_buffer = audio_buffer.clone();
                
                tokio::spawn(async move {
                    // Read chunks
                    let mut buf = [0u8; 4096]; // 1KB buffer (256 floats)
                    loop {
                        match recv_stream.read(&mut buf).await {
                            Ok(Some(n)) => {
                                // Convert bytes to f32
                                let float_count = n / 4;
                                let mut samples = Vec::with_capacity(float_count);
                                for i in 0..float_count {
                                    let start = i * 4;
                                    let end = start + 4;
                                    let val = f32::from_le_bytes(buf[start..end].try_into().unwrap());
                                    samples.push(val);
                                }
                                
                                // Update visualization buffer
                                {
                                    let mut vis_buf = audio_buffer.lock().unwrap();
                                    vis_buf.extend_from_slice(&samples);
                                    if vis_buf.len() > 2048 {
                                        let excess = vis_buf.len() - 2048;
                                        vis_buf.drain(0..excess);
                                    }
                                }

                                if let Err(_) = tx.send(samples).await {
                                    break;
                                }
                            }
                            Ok(None) => {
                                break;
                            }
                            Err(_) => {
                                break;
                            }
                        }
                    }
                });
            }
        });
    }

    Ok(())
}
