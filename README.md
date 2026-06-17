![Main Banner](./images/Banner.png)

# ✨ Rozed
![Zed](https://img.shields.io/badge/Zed-extension-purple)
![Roblox Studio](https://img.shields.io/badge/Roblox%20Studio-plugin-blue)
![Rust](https://img.shields.io/badge/Rust-1.78+-orange)

A sync bridge between [Zed](https://zed.dev) and Roblox Studio. Edit Luau scripts locally in Zed and watch them appear in Studio instantly — no copy-paste, no manual reimport.

---

## 📦 What's Included

| Component | Path | Description |
| --- | --- | --- |
| rozed binary | `crates/rozed-core/` | Local HTTP server that watches your files and broadcasts changes |
| Roblox plugin | `roblox-plugin/rozed.luau` | Studio dock panel — connects, receives pushes, and handles pulls |
| Zed extension | `crates/rozed-zed/` | WASM extension that auto-starts the server when you open a project |

---

## 🚀 How It Works

```
Zed (save file)
  └─→  rozed server  (file watcher detects change, HTTP push)
          └─→  Studio plugin  (polls /events/poll every 500ms, applies script)
```

Edits flow one direction automatically (Zed → Studio) whenever you save. You can also pull scripts from Studio back to disk using the **Pull from Zed** button in the panel.

---

## ⚙️ Setup

### 1. Build the server

Requires [Rust](https://rustup.rs/).

```bash
cargo build -p rozed-core --release
```

The binary lands at `target/release/rozed.exe` (Windows) or `target/release/rozed` (macOS/Linux).

### 2. Create `rozed.toml`

Place this file in the **root of your Luau project** (the folder you open in Zed):

```toml
port = 5500
push_on_save = true
sync_interval_ms = 1000

[mappings]
"src/shared" = "ReplicatedStorage/Shared"
"src/server" = "ServerScriptService"
"src/client" = "StarterPlayerScripts"
```

**Mappings** connect local folders to Roblox paths. The left side is relative to `rozed.toml`; the right side is the full Roblox path from `game`.

### 3. Install the Roblox plugin

Copy `roblox-plugin/rozed.luau` into your Roblox plugins folder:

```
Windows:  %LOCALAPPDATA%\Roblox\Plugins\rozed.luau
macOS:    ~/Documents/Roblox/Plugins/rozed.luau
```

Restart Roblox Studio. The **ROZED** panel will appear in the dock on the right.

### 4. Start the server

Open a terminal in your project folder (where `rozed.toml` is) and run:

```bash
# dev build (slower, no optimisations)
cargo run -p rozed-core

# release build (use this for everyday work)
./target/release/rozed
```

You should see:

```
[INFO] rozed starting on port 5500
[INFO] mapping: src/shared -> ReplicatedStorage/Shared
[CONNECTED] listening on http://127.0.0.1:5500
```

### 5. Connect in Studio

Click **Connect** in the ROZED panel. The status line changes to `[CONNECTED] localhost:5500`.

Now save any `.luau` file in Zed — it appears in Studio immediately.

---

## 🗂️ File Naming Convention

Rozed infers the Roblox script type from the file suffix:

| Local file | Roblox type |
| --- | --- |
| `foo.module.luau` | `ModuleScript` |
| `foo.server.luau` | `Script` |
| `foo.client.luau` | `LocalScript` |

Scripts are created (or updated in-place) at the mapped Roblox path. For example, `src/shared/math.module.luau` with mapping `src/shared → ReplicatedStorage/Shared` becomes `ReplicatedStorage/Shared/math` as a `ModuleScript`.

---

## 🔧 rozed.toml Reference

```toml
port            = 5500    # port the HTTP server listens on
push_on_save    = true    # watch files and push to Studio on save
sync_interval_ms = 1000  # polling interval for internal tasks (ms)

[mappings]
# "local/path" = "Roblox/Path"
"src/shared"  = "ReplicatedStorage/Shared"
"src/server"  = "ServerScriptService"
"src/client"  = "StarterPlayerScripts"
```

Multiple mappings are supported. Unmapped files are silently ignored.

---

## ⬇️ Pulling from Studio

To sync scripts **from Studio back to disk**:

1. Make sure the server is running and the panel is connected.
2. Click **Pull from Zed** in the ROZED panel.
3. Rozed walks every mapped Roblox path, collects scripts, and compares them to your local files.
4. If a file differs, a **conflict dialog** appears — choose **Overwrite** to replace the local file or **Skip** to leave it untouched.
5. Accepted files are written to disk (a `.backup` copy of the original is kept alongside).

---

## ⚠️ Common Errors

> `rozed.toml not found at ...`

You ran the binary from the wrong directory. `rozed` must be run from the folder that contains `rozed.toml`.

> `[ERROR] Could not reach rozed on port 5500. Is it running?`

The Studio plugin tried to connect but got no response. Make sure the rozed binary is running and listening on the same port configured in `rozed.toml`.

> `[ERROR] Http requests are not enabled. Enable via game settings`

HTTP requests are disabled in your Studio place. Go to **Home → Game Settings → Security** and enable **Allow HTTP Requests**.

> `[ERROR] Lost connection: ...`

The server stopped while the plugin was polling. Restart the binary and click **Connect** again.

---

## 🧩 Zed Extension (optional)

The Zed extension bundles the binary and the Roblox plugin together for distribution. Install it as a dev extension from Zed's extension panel by pointing it at the `crates/rozed-zed/` folder.

> **Note:** The extension currently requires you to start the binary manually (`cargo run -p rozed-core` or `rozed.exe`). Auto-launch on workspace open is planned.

---

## 📁 Project Structure

```
rozed/
├── crates/
│   ├── rozed-core/          rozed binary (Rust, axum, notify)
│   │   └── src/
│   │       ├── main.rs      entry point, wires all modules
│   │       ├── server.rs    axum HTTP server + /events/poll endpoint
│   │       ├── watcher.rs   file watcher, broadcasts script-pushed events
│   │       ├── pull.rs      conflict detection, backup, file write
│   │       ├── events.rs    serde event types
│   │       ├── mapping.rs   local path ↔ Roblox path resolution
│   │       └── config.rs    rozed.toml parsing
│   └── rozed-zed/           Zed WASM extension
│       ├── src/lib.rs
│       ├── extension.toml
│       └── assets/
│           └── rozed.luau   bundled plugin copy
├── roblox-plugin/
│   └── rozed.luau           Roblox Studio dock panel plugin
└── rozed.toml               your project config (not committed)
```
