mod config;
mod events;
mod mapping;
mod pull;
mod server;
mod watcher;

use std::sync::Arc;
use anyhow::Result;
use tokio::sync::broadcast;
use config::Config;
use server::AppState;

#[tokio::main]
async fn main() -> Result<()> {
    let project_root = std::env::current_dir()?;
    let config = Config::load(&project_root)?;

    println!("[INFO] rozed starting on port {}", config.port());
    println!(
        "[INFO] push_on_save={}, sync_interval_ms={}",
        config.push_on_save(),
        config.sync_interval_ms()
    );
    for (local, roblox) in &config.mappings {
        println!("[INFO] mapping: {} -> {}", local, roblox);
    }

    let (tx, _) = broadcast::channel::<events::Event>(256);

    let poll_queue: Arc<std::sync::Mutex<Vec<events::Event>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));

    let state = Arc::new(AppState {
        project_root: project_root.clone(),
        mappings: config.mappings.clone(),
        tx: tx.clone(),
        poll_queue: poll_queue.clone(),
    });

    // Mirror all broadcast events into the poll queue for HTTP polling clients
    let pq = poll_queue.clone();
    let mut rx = tx.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            if let Ok(mut q) = pq.lock() {
                q.push(event);
            }
        }
    });

    if config.push_on_save() {
        let root = project_root.clone();
        let mappings = config.mappings.clone();
        let tx2 = tx.clone();
        tokio::spawn(async move {
            if let Err(e) = watcher::start_watcher(root, mappings, tx2).await {
                eprintln!("[ERROR] watcher: {}", e);
            }
        });
    }

    let interval_ms = config.sync_interval_ms();
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_millis(interval_ms));
        loop {
            interval.tick().await;
        }
    });

    let addr = format!("127.0.0.1:{}", config.port());
    println!("[CONNECTED] listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, server::router(state)).await?;

    Ok(())
}
