use std::sync::{Arc, Mutex};
use std::time::Duration;
use futures::SinkExt;
use futures::StreamExt;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use log::{info, error};
use tokio::sync::broadcast;
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

#[derive(Debug)]
struct Score {
    england: u32,
    spain: u32,
}

#[tokio::main]
async fn main() {
    // Initialize logger
    env_logger::init();

    let addr = "127.0.0.1:8080";
    let listener = TcpListener::bind(addr).await.unwrap();
    info!("WebSocket server running at {}", addr);

    // Shared score state
    let score = Arc::new(Mutex::new(Score { england: 0, spain: 0 }));

    // Broadcast channel to notify all clients of score updates
    let (tx, _rx) = broadcast::channel(10);

    // Task to update the score periodically
    let score_clone = Arc::clone(&score);
    let tx_clone = tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(20));
        let mut rng = StdRng::from_entropy();

        loop {
            interval.tick().await;
            let mut score = score_clone.lock().unwrap();
            let team = rng.gen_range(0..2);
            if team == 0 {
                score.england += 1;
            } else {
                score.spain += 1;
            }

            let message = format!("Score: {} - {} - England vs Spain", score.england, score.spain);
            let _ = tx_clone.send(message.clone()); // Broadcast the updated score
            info!("Updated score: {}", message);
        }
    });

    while let Ok((stream, _)) = listener.accept().await {
        let score_clone = Arc::clone(&score);
        let rx = tx.subscribe();
        tokio::spawn(handle_client(stream, score_clone, rx));
    }
}

async fn handle_client(stream: tokio::net::TcpStream, score: Arc<Mutex<Score>>, mut rx: broadcast::Receiver<String>) {
    let ws_stream = accept_async(stream).await.unwrap();
    let (mut write, mut read) = ws_stream.split();

    // Log new connection
    info!("New WebSocket connection established");

    // Send the initial score to the client
    let initial_message = {
        let score = score.lock().unwrap();
        format!("Score: {} - {} - England vs Spain", score.england, score.spain)
    };

    if write.send(Message::Text(initial_message.clone())).await.is_err() {
        error!("Error sending initial score to client");
        return;
    }
    info!("Sent initial score: {}", initial_message);

    // Create a task to send updates to the client
    tokio::spawn(async move {
        while let Ok(message) = rx.recv().await {
            if write.send(Message::Text(message.clone())).await.is_err() {
                break;
            }
            info!("Sent message: {}", message);
        }
    });

    // Handle incoming messages (if necessary)
    while let Some(Ok(msg)) = read.next().await {
        info!("Received message: {:?}", msg);
        // Handle incoming messages here if needed
    }

    // Log when connection is closed
    info!("WebSocket connection closed");
}

