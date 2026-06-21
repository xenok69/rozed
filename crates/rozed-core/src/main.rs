mod config;
mod events;
mod mapping;
mod pull;
mod server;
mod sync;
mod watcher;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
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

    eprintln!("[INFO] rozed starting on port 5500");
    eprintln!(
        "[INFO] push_on_save={}, sync_interval_ms={}",
        config.push_on_save(),
        config.sync_interval_ms()
    );
    for (local, roblox) in &config.mappings {
        eprintln!("[INFO] mapping: {} -> {}", local, roblox);
    }

    let (tx, _) = broadcast::channel::<events::Event>(256);

    let poll_queue: Arc<Mutex<Vec<events::Event>>> =
        Arc::new(Mutex::new(Vec::new()));

    let mappings_shared = Arc::new(RwLock::new(config.mappings.clone()));

    let state = Arc::new(AppState {
        project_root: project_root.clone(),
        mappings: mappings_shared.clone(),
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

    let watcher_holder: Arc<Mutex<Option<watcher::WatchHandle>>> =
        Arc::new(Mutex::new(None));

    if config.push_on_save() {
        match watcher::start_watcher(project_root.clone(), config.mappings.clone(), tx.clone()) {
            Ok(handle) => *watcher_holder.lock().unwrap() = Some(handle),
            Err(e) => eprintln!("[ERROR] watcher: {}", e),
        }
    }

    // Watch rozed.toml so mappings and the file watcher update on save
    {
        let root = project_root.clone();
        let reload_tx = tx.clone();
        let reload_mappings = mappings_shared.clone();
        let reload_holder = watcher_holder.clone();
        tokio::task::spawn_blocking(move || {
            run_config_watcher(root, reload_tx, reload_mappings, reload_holder);
        });
    }

    let interval_ms = config.sync_interval_ms();
    let sync_root = project_root.clone();
    let sync_mappings = mappings_shared.clone();
    let sync_tx = tx.clone();
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_millis(interval_ms));
        loop {
            interval.tick().await;
            let mappings = sync_mappings.read().unwrap().clone();
            let root = sync_root.clone();
            let tx2 = sync_tx.clone();
            let window = std::time::Duration::from_millis(interval_ms);
            tokio::task::spawn_blocking(move || {
                sync::push_changed_files(&root, &mappings, &tx2, window);
            });
        }
    });

    let addr = "127.0.0.1:5500".to_string();
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    eprintln!("[CONNECTED] listening on http://{}", addr);
    axum::serve(listener, server::router(state)).await?;

    Ok(())
}

fn run_config_watcher(
    project_root: PathBuf,
    tx: broadcast::Sender<events::Event>,
    shared_mappings: Arc<RwLock<HashMap<String, String>>>,
    watcher_holder: Arc<Mutex<Option<watcher::WatchHandle>>>,
) {
    use notify::{
        Config as NotifyConfig, Event as NotifyEvent, EventKind,
        RecommendedWatcher, RecursiveMode, Watcher,
    };

    let (ntx, nrx) = std::sync::mpsc::channel();
    let mut config_watcher = match RecommendedWatcher::new(ntx, NotifyConfig::default()) {
        Ok(w) => w,
        Err(e) => { eprintln!("[ERROR] config watcher init: {}", e); return; }
    };

    let config_file = project_root.join("rozed.toml");
    if let Err(e) = config_watcher.watch(&config_file, RecursiveMode::NonRecursive) {
        eprintln!("[ERROR] watching rozed.toml: {}", e);
        return;
    }

    let _config_watcher = config_watcher;

    for result in nrx {
        if let Ok(NotifyEvent { kind: EventKind::Modify(_) | EventKind::Create(_), .. }) = result {
            match Config::load(&project_root) {
                Ok(new_config) => {
                    eprintln!("[INFO] rozed.toml reloaded");
                    *shared_mappings.write().unwrap() = new_config.mappings.clone();

                    let new_handle = if new_config.push_on_save() {
                        watcher::start_watcher(
                            project_root.clone(),
                            new_config.mappings,
                            tx.clone(),
                        ).map_err(|e| eprintln!("[ERROR] restarting watcher: {}", e)).ok()
                    } else {
                        None
                    };
                    *watcher_holder.lock().unwrap() = new_handle;
                }
                Err(e) => eprintln!("[ERROR] reloading rozed.toml: {}", e),
            }
        }
    }
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
