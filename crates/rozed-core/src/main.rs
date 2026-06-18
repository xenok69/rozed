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
    // Respond to Zed's LSP initialize handshake in the background.
    // rozed is not an LSP server, but Zed spawns it as one; answering
    // initialize prevents Zed from killing the process on timeout.
    tokio::task::spawn_blocking(lsp_handshake);

    let project_root = std::env::current_dir()?;
    let config = Config::load(&project_root)?;

    eprintln!("[INFO] rozed starting on port {}", config.port());
    eprintln!(
        "[INFO] push_on_save={}, sync_interval_ms={}",
        config.push_on_save(),
        config.sync_interval_ms()
    );
    for (local, roblox) in &config.mappings {
        eprintln!("[INFO] mapping: {} -> {}", local, roblox);
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
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    eprintln!("[CONNECTED] listening on http://{}", addr);
    axum::serve(listener, server::router(state)).await?;

    Ok(())
}

fn lsp_handshake() {
    use std::io::{BufRead, BufReader, Read, Write};
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut stdout = stdout.lock();

    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let line = line.trim_end();
        if !line.starts_with("Content-Length:") {
            continue;
        }
        let Ok(len) = line["Content-Length:".len()..].trim().parse::<usize>() else {
            continue;
        };

        let mut blank = String::new();
        let _ = reader.read_line(&mut blank);

        let mut body = vec![0u8; len];
        if reader.read_exact(&mut body).is_err() {
            break;
        }
        let body = String::from_utf8_lossy(&body);

        if body.contains("\"initialize\"") {
            let id = extract_lsp_id(&body).unwrap_or(1);
            let result = format!(
                r#"{{"jsonrpc":"2.0","id":{id},"result":{{"capabilities":{{}}}}}}"#
            );
            let _ = write!(stdout, "Content-Length: {}\r\n\r\n{}", result.len(), result);
            let _ = stdout.flush();
        } else if body.contains("\"shutdown\"") {
            let id = extract_lsp_id(&body).unwrap_or(1);
            let result = format!(r#"{{"jsonrpc":"2.0","id":{id},"result":null}}"#);
            let _ = write!(stdout, "Content-Length: {}\r\n\r\n{}", result.len(), result);
            let _ = stdout.flush();
            break;
        }
    }
}

fn extract_lsp_id(json: &str) -> Option<i64> {
    let pos = json.find("\"id\"")?;
    let rest = json[pos + 4..].trim_start_matches(|c: char| c == ' ' || c == ':');
    rest.split(|c: char| !c.is_ascii_digit() && c != '-')
        .next()
        .and_then(|s| s.parse().ok())
}
