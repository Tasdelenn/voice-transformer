use anyhow::Result;
use clap::Parser;
use tokio::sync::mpsc;
use voice_transformer_lib::{audio, network};
use std::net::SocketAddr;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Listen port
    #[arg(long, default_value = "5000")]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let addr: SocketAddr = format!("0.0.0.0:{}", args.port).parse()?;
    
    println!("Starting Receiver listening on {}", addr);

    // 1. Setup Audio Output
    let (tx, rx) = mpsc::channel::<Vec<f32>>(100);
    let _audio_stream = audio::AudioStream::setup_output(rx)?;
    println!("Audio output ready");

    // 2. Setup QUIC Server
    let (endpoint, _cert) = network::make_server_endpoint(addr)?;
    
    println!("Waiting for connections...");

    // 3. Accept Loop
    while let Some(connecting) = endpoint.accept().await {
        println!("New connection incoming...");
        let tx = tx.clone();
        tokio::spawn(async move {
            let connection = match connecting.await {
                Ok(conn) => conn,
                Err(e) => {
                    eprintln!("Connection failed: {}", e);
                    return;
                }
            };
            println!("Connection established from {}", connection.remote_address());

            while let Ok(mut recv_stream) = connection.accept_uni().await {
                println!("Stream accepted");
                let tx = tx.clone();
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
                                if let Err(_) = tx.send(samples).await {
                                    break;
                                }
                            }
                            Ok(None) => {
                                println!("Stream finished");
                                break;
                            }
                            Err(e) => {
                                eprintln!("Stream error: {}", e);
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
