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
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tokio::sync::broadcast;
use config::Config;
use server::AppState;

const PORT: u16 = 5500;
const ADDR: &str = "127.0.0.1:5500";

#[derive(Parser)]
#[command(name = "rozed", about = "Roblox-Zed sync daemon")]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Start the sync server
    Start {
        /// Run in the background (detached)
        #[arg(short = 'd', long)]
        detached: bool,
        /// Open in a new terminal window
        #[arg(short = 't', long)]
        terminal: bool,
    },
    /// Stop the running background daemon
    Stop,
    /// Show daemon status and active mappings
    Status,
    /// Push all mapped files to Roblox immediately
    Build,
    /// Run the server loop (used internally by -d and -t)
    #[command(hide = true)]
    Serve,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Cmd::Start { detached: true, .. }) => cmd_start_detached()?,
        Some(Cmd::Start { terminal: true, .. }) => cmd_start_terminal()?,
        Some(Cmd::Start { .. }) | Some(Cmd::Serve) | None => run_server().await?,
        Some(Cmd::Stop) => cmd_stop()?,
        Some(Cmd::Status) => cmd_status().await?,
        Some(Cmd::Build) => cmd_build().await?,
    }
    Ok(())
}

// ── Project root discovery ────────────────────────────────────────────────────

fn find_project_root() -> Result<PathBuf> {
    let mut dir = std::env::current_dir()?;
    loop {
        if dir.join("rozed.toml").exists() {
            return Ok(dir);
        }
        match dir.parent().map(|p| p.to_path_buf()) {
            Some(parent) => dir = parent,
            None => {
                return Err(anyhow::anyhow!(
                    "[ERROR] no rozed.toml found in this directory or any parent"
                ))
            }
        }
    }
}

fn pid_path(project_root: &Path) -> PathBuf {
    project_root.join(".rozed").join("rozed.pid")
}

// ── Port availability check ───────────────────────────────────────────────────

fn is_server_running() -> bool {
    std::net::TcpStream::connect(ADDR).is_ok()
}

// ── start -d ──────────────────────────────────────────────────────────────────

fn cmd_start_detached() -> Result<()> {
    if is_server_running() {
        eprintln!("[INFO] rozed is already running on port {}", PORT);
        return Ok(());
    }
    let exe = std::env::current_exe().context("[ERROR] cannot locate own executable")?;
    spawn_detached(&exe)?;
    eprintln!("[INFO] rozed daemon started — run 'rozed stop' to stop");
    Ok(())
}

#[cfg(windows)]
fn spawn_detached(exe: &Path) -> Result<()> {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x00000008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
    std::process::Command::new(exe)
        .arg("serve")
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .spawn()
        .context("[ERROR] failed to spawn detached process")?;
    Ok(())
}

#[cfg(not(windows))]
fn spawn_detached(exe: &Path) -> Result<()> {
    std::process::Command::new(exe)
        .arg("serve")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("[ERROR] failed to spawn daemon")?;
    Ok(())
}

// ── start -t ──────────────────────────────────────────────────────────────────

fn cmd_start_terminal() -> Result<()> {
    if is_server_running() {
        eprintln!("[INFO] rozed is already running on port {}", PORT);
        return Ok(());
    }
    let exe = std::env::current_exe().context("[ERROR] cannot locate own executable")?;
    spawn_in_terminal(&exe)?;
    eprintln!("[INFO] rozed started in new terminal window");
    Ok(())
}

#[cfg(windows)]
fn spawn_in_terminal(exe: &Path) -> Result<()> {
    std::process::Command::new("cmd")
        .args([
            "/c",
            "start",
            "rozed",
            exe.to_str().unwrap_or("rozed"),
            "serve",
        ])
        .spawn()
        .context("[ERROR] failed to open a new terminal window")?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn spawn_in_terminal(exe: &Path) -> Result<()> {
    let script = format!(
        r#"tell application "Terminal" to do script "{} serve""#,
        exe.display()
    );
    std::process::Command::new("osascript")
        .args(["-e", &script])
        .spawn()
        .context("[ERROR] failed to open Terminal.app")?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn spawn_in_terminal(exe: &Path) -> Result<()> {
    let exe_str = exe.to_str().unwrap_or("rozed");
    let attempts: &[(&str, &[&str])] = &[
        ("x-terminal-emulator", &["-e", exe_str, "serve"]),
        ("gnome-terminal", &["--", exe_str, "serve"]),
        ("konsole", &["-e", exe_str, "serve"]),
        ("xterm", &["-e", exe_str, "serve"]),
    ];
    for (term, args) in attempts {
        if std::process::Command::new(term).args(*args).spawn().is_ok() {
            return Ok(());
        }
    }
    Err(anyhow::anyhow!(
        "[ERROR] no terminal emulator found (tried x-terminal-emulator, gnome-terminal, konsole, xterm)"
    ))
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
fn spawn_in_terminal(_exe: &Path) -> Result<()> {
    Err(anyhow::anyhow!("[ERROR] -t not supported on this platform"))
}

// ── stop ──────────────────────────────────────────────────────────────────────

fn cmd_stop() -> Result<()> {
    let project_root = find_project_root()?;
    let pid_file = pid_path(&project_root);
    let content = std::fs::read_to_string(&pid_file)
        .context("[ERROR] daemon is not running (no PID file found)")?;
    let pid: u32 = content
        .trim()
        .parse()
        .context("[ERROR] corrupt PID file")?;
    kill_process(pid)?;
    std::fs::remove_file(&pid_file).ok();
    eprintln!("[INFO] rozed stopped");
    Ok(())
}

#[cfg(windows)]
fn kill_process(pid: u32) -> Result<()> {
    std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output()
        .context("[ERROR] taskkill failed")?;
    Ok(())
}

#[cfg(not(windows))]
fn kill_process(pid: u32) -> Result<()> {
    std::process::Command::new("kill")
        .arg(pid.to_string())
        .status()
        .context("[ERROR] kill failed")?;
    Ok(())
}

// ── status ────────────────────────────────────────────────────────────────────

async fn cmd_status() -> Result<()> {
    match http_get(ADDR, "/status").await {
        Ok(body) => {
            eprintln!("[OK] rozed is running on port {}", PORT);
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(m) = v.get("mappings").and_then(|m| m.as_object()) {
                    for (k, v) in m {
                        eprintln!("[INFO] mapping: {} -> {}", k, v.as_str().unwrap_or("?"));
                    }
                }
            }
        }
        Err(_) => eprintln!("[INFO] rozed is not running"),
    }
    Ok(())
}

// ── build ─────────────────────────────────────────────────────────────────────

async fn cmd_build() -> Result<()> {
    match http_post(ADDR, "/build", "{}").await {
        Ok(_) => eprintln!("[OK] build triggered — all files pushed to Roblox"),
        Err(_) => eprintln!(
            "[ERROR] could not reach rozed — run 'rozed start' first"
        ),
    }
    Ok(())
}

// ── raw HTTP helpers (no reqwest) ─────────────────────────────────────────────

async fn http_get(addr: &str, path: &str) -> Result<String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = tokio::net::TcpStream::connect(addr).await?;
    let req = format!("GET {} HTTP/1.0\r\nHost: {}\r\n\r\n", path, addr);
    stream.write_all(req.as_bytes()).await?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await?;
    let resp = String::from_utf8_lossy(&buf).into_owned();
    Ok(resp.split("\r\n\r\n").nth(1).unwrap_or("").to_string())
}

async fn http_post(addr: &str, path: &str, body: &str) -> Result<String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = tokio::net::TcpStream::connect(addr).await?;
    let req = format!(
        "POST {} HTTP/1.0\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        path, addr, body.len(), body
    );
    stream.write_all(req.as_bytes()).await?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await?;
    let resp = String::from_utf8_lossy(&buf).into_owned();
    Ok(resp.split("\r\n\r\n").nth(1).unwrap_or("").to_string())
}

// ── server loop ───────────────────────────────────────────────────────────────

async fn run_server() -> Result<()> {
    let project_root = find_project_root()?;
    let config = Config::load(&project_root)?;

    // Write PID file for `rozed stop`
    let pid_dir = project_root.join(".rozed");
    std::fs::create_dir_all(&pid_dir)?;
    std::fs::write(pid_dir.join("rozed.pid"), std::process::id().to_string())?;

    eprintln!("[INFO] rozed starting on port {}", PORT);
    eprintln!(
        "[INFO] push_on_save={}, sync_interval_ms={}",
        config.push_on_save(),
        config.sync_interval_ms()
    );
    for (local, roblox) in &config.mappings {
        eprintln!("[INFO] mapping: {} -> {}", local, roblox);
    }

    let (tx, _) = broadcast::channel::<events::Event>(256);
    let poll_queue: Arc<Mutex<Vec<events::Event>>> = Arc::new(Mutex::new(Vec::new()));
    let mappings_shared = Arc::new(RwLock::new(config.mappings.clone()));

    let state = Arc::new(AppState {
        project_root: project_root.clone(),
        mappings: mappings_shared.clone(),
        tx: tx.clone(),
        poll_queue: poll_queue.clone(),
    });

    // Mirror broadcast into poll queue for HTTP polling clients
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

    // Hot-reload rozed.toml on save
    {
        let root = project_root.clone();
        let reload_tx = tx.clone();
        let reload_mappings = mappings_shared.clone();
        let reload_holder = watcher_holder.clone();
        tokio::task::spawn_blocking(move || {
            run_config_watcher(root, reload_tx, reload_mappings, reload_holder);
        });
    }

    // Interval sync
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

    let listener = tokio::net::TcpListener::bind(ADDR).await?;
    eprintln!("[CONNECTED] listening on http://{}", ADDR);

    // Serve until Ctrl-C, then clean up PID file
    tokio::select! {
        result = axum::serve(listener, server::router(state)) => result?,
        _ = tokio::signal::ctrl_c() => {
            eprintln!("[INFO] rozed shutting down");
        }
    }
    std::fs::remove_file(pid_path(&project_root)).ok();
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
        Err(e) => {
            eprintln!("[ERROR] config watcher init: {}", e);
            return;
        }
    };

    let config_file = project_root.join("rozed.toml");
    if let Err(e) = config_watcher.watch(&config_file, RecursiveMode::NonRecursive) {
        eprintln!("[ERROR] watching rozed.toml: {}", e);
        return;
    }
    let _config_watcher = config_watcher;

    for result in nrx {
        if let Ok(NotifyEvent {
            kind: EventKind::Modify(_) | EventKind::Create(_),
            ..
        }) = result
        {
            match Config::load(&project_root) {
                Ok(new_config) => {
                    eprintln!("[INFO] rozed.toml reloaded");
                    *shared_mappings.write().unwrap() = new_config.mappings.clone();
                    let new_handle = if new_config.push_on_save() {
                        watcher::start_watcher(
                            project_root.clone(),
                            new_config.mappings,
                            tx.clone(),
                        )
                        .map_err(|e| eprintln!("[ERROR] restarting watcher: {}", e))
                        .ok()
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
