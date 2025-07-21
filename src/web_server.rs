use std::sync::Arc;
use tokio::sync::Mutex;
use warp::Filter;
use serde::{Deserialize, Serialize};
use warp::ws::Message;
use futures_util::StreamExt;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FFTData {
    pub r#type: String,
    pub input_spectrum: Vec<f32>,
    pub output_spectrum: Vec<f32>,
    pub sample_rate: f32,
    pub fft_size: usize,
}

pub type WebSocketSender = Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<Message>>>>;

pub async fn start_web_server(
    fft_sender: WebSocketSender,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Serve static files
    let static_files = warp::path::end()
        .and(warp::get())
        .and(warp::fs::file("web/index.html"))
        .or(warp::path("style.css")
            .and(warp::get())
            .and(warp::fs::file("web/style.css")))
        .or(warp::path("app.js")
            .and(warp::get())
            .and(warp::fs::file("web/app.js")));

    // WebSocket route
    let websocket = warp::path("ws")
        .and(warp::ws())
        .and(with_sender(fft_sender))
        .map(|ws: warp::ws::Ws, sender| {
            ws.on_upgrade(move |socket| handle_websocket(socket, sender))
        });

    let routes = static_files.or(websocket);

    println!("🌐 Web server starting at http://localhost:3030");
    println!("📊 Open your browser to see the audio visualization!");

    warp::serve(routes)
        .run(([127, 0, 0, 1], 3030))
        .await;

    Ok(())
}

fn with_sender(
    sender: WebSocketSender,
) -> impl Filter<Extract = (WebSocketSender,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || sender.clone())
}

async fn handle_websocket(
    ws: warp::ws::WebSocket,
    global_sender: WebSocketSender,
) {
    let (tx, mut rx) = ws.split();
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();

    // Store the sender for broadcasting
    {
        let mut global = global_sender.lock().await;
        *global = Some(sender);
    }

    // Handle incoming messages (if any)
    let recv_task = tokio::spawn(async move {
        while let Some(result) = rx.next().await {
            match result {
                Ok(msg) => {
                    if msg.is_close() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // Handle outgoing messages
    let send_task = tokio::spawn(async move {
        use futures_util::SinkExt;
        let mut tx = tx;
        while let Some(message) = receiver.recv().await {
            if tx.send(message).await.is_err() {
                break;
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = recv_task => {},
        _ = send_task => {},
    }

    // Clean up
    {
        let mut global = global_sender.lock().await;
        *global = None;
    }
}

pub async fn broadcast_fft_data(
    sender: &WebSocketSender,
    input_spectrum: Vec<f32>,
    output_spectrum: Vec<f32>,
    sample_rate: f32,
    fft_size: usize,
) {
    let data = FFTData {
        r#type: "fft_data".to_string(),
        input_spectrum,
        output_spectrum,
        sample_rate,
        fft_size,
    };

    if let Ok(json) = serde_json::to_string(&data) {
let message = warp::ws::Message::text(json);
        
        let sender_guard = sender.lock().await;
        if let Some(ref tx) = *sender_guard {
            let _ = tx.send(message);
        }
    }
}
