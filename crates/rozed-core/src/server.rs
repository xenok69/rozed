use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use axum::{
    Router,
    extract::{State, WebSocketUpgrade},
    extract::ws::{Message, WebSocket},
    response::IntoResponse,
    routing::{get, post},
    Json,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::broadcast;
use crate::events::{Event, PullRequest};
use crate::pull::{check_conflict, write_script};

pub struct AppState {
    pub project_root: PathBuf,
    pub mappings: HashMap<String, String>,
    pub tx: broadcast::Sender<Event>,
    pub poll_queue: Arc<std::sync::Mutex<Vec<Event>>>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/status", get(status_handler))
        .route("/push", post(push_handler))
        .route("/pull", post(pull_handler))
        .route("/pull/confirm", post(pull_confirm_handler))
        .route("/init", post(init_handler))
        .route("/events", get(ws_handler))
        .route("/events/poll", get(poll_handler))
        .with_state(state)
}

async fn status_handler(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "status": "running",
        "mappings": state.mappings,
    }))
}

async fn push_handler() -> Json<Value> {
    Json(json!({ "ok": true }))
}

async fn pull_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PullRequest>,
) -> Json<Value> {
    let files: Vec<_> = body.files.iter()
        .map(|s| check_conflict(s, &state.project_root, &state.mappings))
        .collect();
    state.tx.send(Event::PullReady { files }).ok();
    Json(json!({ "ok": true }))
}

async fn pull_confirm_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PullRequest>,
) -> Json<Value> {
    let mut errors: Vec<String> = vec![];
    for script in &body.files {
        let file = check_conflict(script, &state.project_root, &state.mappings);
        if let Err(e) = write_script(&file, &state.project_root, &state.mappings) {
            errors.push(e.to_string());
        }
    }
    if errors.is_empty() {
        Json(json!({ "ok": true }))
    } else {
        Json(json!({ "ok": false, "errors": errors }))
    }
}

async fn init_handler(State(state): State<Arc<AppState>>) -> Json<Value> {
    state.tx.send(Event::StructureOk).ok();
    Json(json!({ "ok": true }))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.tx.subscribe();
    let (mut sender, _receiver) = socket.split();
    while let Ok(event) = rx.recv().await {
        let msg = serde_json::to_string(&event).unwrap_or_default();
        if sender.send(Message::Text(msg.into())).await.is_err() {
            break;
        }
    }
}

async fn poll_handler(State(state): State<Arc<AppState>>) -> Json<Value> {
    let events = {
        let mut queue = state.poll_queue.lock().unwrap();
        queue.drain(..).collect::<Vec<_>>()
    };
    let json_events: Vec<Value> = events.iter()
        .filter_map(|e| serde_json::to_value(e).ok())
        .collect();
    Json(Value::Array(json_events))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum_test::TestServer;
    use std::collections::HashMap;
    use tokio::sync::broadcast;

    fn test_state() -> Arc<AppState> {
        let (tx, _) = broadcast::channel(100);
        Arc::new(AppState {
            project_root: std::env::temp_dir(),
            mappings: HashMap::new(),
            tx,
            poll_queue: Arc::new(std::sync::Mutex::new(Vec::new())),
        })
    }

    #[tokio::test]
    async fn test_status_returns_running() {
        let app = router(test_state());
        let server = TestServer::new(app).unwrap();
        let resp = server.get("/status").await;
        resp.assert_status_ok();
        let body: serde_json::Value = resp.json();
        assert_eq!(body["status"], "running");
    }

    #[tokio::test]
    async fn test_pull_confirm_writes_non_conflicting_file() {
        use tempfile::TempDir;
        use std::fs;

        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("src/shared")).unwrap();

        let mut mappings = HashMap::new();
        mappings.insert("src/shared".into(), "ReplicatedStorage/Shared".into());

        let (tx, _) = broadcast::channel(100);
        let state = Arc::new(AppState {
            project_root: dir.path().to_path_buf(),
            mappings,
            tx,
            poll_queue: Arc::new(std::sync::Mutex::new(Vec::new())),
        });

        let app = router(state);
        let server = TestServer::new(app).unwrap();
        let body = serde_json::json!({
            "files": [{
                "roblox_path": "ReplicatedStorage/Shared/foo",
                "name": "foo",
                "kind": "ModuleScript",
                "source": "return 42"
            }]
        });
        let resp = server.post("/pull/confirm").json(&body).await;
        resp.assert_status_ok();
        assert!(dir.path().join("src/shared/foo.module.luau").exists());
    }
}
