# Architecture

Rozed has three components that communicate over localhost HTTP.

```
┌─────────────────────────────────────────────────┐
│  Zed (editor)                                   │
│  saves foo.module.luau                          │
└────────────────────┬────────────────────────────┘
                     │  file system event
                     ▼
┌─────────────────────────────────────────────────┐
│  rozed  (Rust binary, axum + tokio)             │
│                                                 │
│  ┌──────────┐    broadcast::Sender<Event>       │
│  │ watcher  │ ──────────────────────────────┐   │
│  └──────────┘                               │   │
│                                             ▼   │
│  ┌─────────────────────────────────────────┐    │
│  │  poll subscriber task                   │    │
│  │  pushes events into poll_queue (Mutex)  │    │
│  └─────────────────────────────────────────┘    │
│                                             ▼   │
│  GET /events/poll  ◄────────────────  poll_queue│
│  POST /pull        ──► check_conflict ──► Event │
│  POST /pull/confirm ──► write_script            │
│  GET /status                                    │
└────────────────────┬────────────────────────────┘
                     │  HTTP (localhost:5500)
                     ▼
┌─────────────────────────────────────────────────┐
│  Roblox Studio plugin  (Luau, HttpService)      │
│                                                 │
│  polls GET /events/poll every 500ms             │
│  applies script-pushed → instance.Source       │
│  sends pull-ready conflicts → confirm dialog   │
└─────────────────────────────────────────────────┘
```

---

## Components

### rozed binary (`crates/rozed-core/`)

A single-binary Rust server built on [axum](https://github.com/tokio-rs/axum) and [tokio](https://tokio.rs/).

| Module | Responsibility |
| --- | --- |
| `config.rs` | Parses `rozed.toml` |
| `mapping.rs` | Resolves local file paths to Roblox paths and script types |
| `events.rs` | Defines the `Event` enum (serialised as tagged JSON) |
| `watcher.rs` | Watches mapped folders with `notify`, filters with `.rozedignore` |
| `pull.rs` | Conflict detection, `.backup` creation, file writes |
| `server.rs` | axum router — all HTTP endpoints + WebSocket handler |
| `main.rs` | Wires everything together, starts the poll subscriber task |

### Roblox Studio plugin (`roblox-plugin/rozed.luau`)

A single-file Luau plugin that runs inside Roblox Studio. It uses `HttpService:GetAsync` and `HttpService:PostAsync` — no WebSocket (that API is unavailable in the plugin context).

### Zed extension (`crates/rozed-zed/`)

A WASM (`wasm32-wasip1`) extension compiled from Rust. It installs the binary and the plugin on first activation.

---

## HTTP API

All endpoints are on `http://127.0.0.1:{port}` (default `5500`).

### `GET /status`

Returns the server state and configured mappings.

```json
{
  "status": "running",
  "mappings": {
    "src/shared": "ReplicatedStorage/Shared"
  }
}
```

### `GET /events/poll`

Returns and clears all queued events. The plugin calls this every 500 ms.

```json
[
  {
    "type": "script-pushed",
    "name": "combat",
    "kind": "ModuleScript",
    "roblox_path": "ReplicatedStorage/Shared/combat",
    "source": "return {}"
  }
]
```

Returns an empty array `[]` when there are nothing queued.

### `POST /pull`

Accepts a list of scripts from Studio, checks each against the local file on disk, and queues a `pull-ready` event.

**Request:**
```json
{
  "files": [
    {
      "roblox_path": "ReplicatedStorage/Shared/combat",
      "name": "combat",
      "kind": "ModuleScript",
      "source": "return {}"
    }
  ]
}
```

### `POST /pull/confirm`

Same body as `/pull`. Writes the accepted files to disk (creating `.backup` copies of any files that differ).

### `POST /init`

Broadcasts a `structure-ok` event (used for initial structure validation).

### `GET /events`

WebSocket upgrade endpoint. Kept for potential future clients — the Studio plugin does not use this.

---

## Event Types

Events are serialised as JSON with a `"type"` discriminant field (using serde `tag`). Variant names are `kebab-case`.

| `type` | Fields | Description |
| --- | --- | --- |
| `script-pushed` | `name`, `kind`, `roblox_path`, `source` | A local file was saved and pushed |
| `pull-ready` | `files[]` (each with `name`, `roblox_path`, `kind`, `source`, `conflict`) | Server responded to `/pull` |
| `structure-ok` | — | All mapped Roblox paths exist in the data model |
| `structure-missing` | `paths[]` | One or more mapped paths don't exist yet |
| `error` | `message` | A server-side error occurred |

---

## Push Flow (Zed → Studio)

1. File saved in Zed → `notify` fires a `Modify` or `Create` event.
2. `watcher.rs` checks `.rozedignore`, resolves the path via `mapping.rs`.
3. Broadcasts `Event::ScriptPushed` on the `broadcast::Sender<Event>`.
4. The poll subscriber task receives it and pushes it into `poll_queue`.
5. The Studio plugin's next poll returns it from `GET /events/poll`.
6. `applyScript()` sets `instance.Source` (or creates the instance).

## Pull Flow (Studio → Disk)

1. User clicks **Pull from Zed** in the panel.
2. Plugin walks all mapped Roblox paths, collects scripts, POSTs them to `/pull`.
3. `pull.rs` runs `check_conflict()` — trims and compares source strings.
4. `Event::PullReady` is broadcast → reaches `poll_queue` via subscriber.
5. Plugin polls and gets `pull-ready` event, shows conflict dialogs for differing files.
6. Plugin POSTs confirmed files to `/pull/confirm`.
7. `write_script()` copies original to `.backup`, writes new content.

---

## Why HTTP Polling, Not WebSockets

`HttpService:WebSocketConnect` does not exist in the Roblox Studio plugin API. It is only available inside game server/client scripts. The plugin uses `HttpService:GetAsync` (standard HTTP) instead, polling every 500 ms. The latency is imperceptible for the typical save-and-check workflow.
