# Roblox Studio Plugin

The ROZED panel is a dock widget that connects to the rozed server and handles both incoming pushes and outgoing pulls.

---

## Installation

### Option A — Script plugin (recommended)

Copy `roblox-plugin/rozed.luau` to your Roblox plugins folder:

```
Windows:  %LOCALAPPDATA%\Roblox\Plugins\rozed.luau
macOS:    ~/Documents/Roblox/Plugins/rozed.luau
```

Restart Studio. The plugin loads automatically as a `LocalScript`.

### Option B — Model file (.rbxmx)

If you want to distribute the plugin as a model file:

1. Open Roblox Studio.
2. In the Explorer, create a `Script` or `LocalScript`.
3. Paste the contents of `roblox-plugin/rozed.luau` into it.
4. Right-click the script → **Save to File** → save as `rozed.rbxmx` in the plugins folder.
5. Restart Studio.

Both formats work identically at runtime.

---

## The Panel

The ROZED dock panel opens on the right side of Studio. Use the **ROZED** toolbar button to show or hide it.

| Element | Description |
| --- | --- |
| Status line | Shows `[CONNECTED] localhost:5500` or `[DISCONNECTED]` |
| Connect / Disconnect | Starts or stops polling |
| Pull from Zed | Pulls all scripts from mapped Roblox paths to disk |
| Log area | Scrollable output of all events and errors |

---

## Connecting

Before clicking **Connect**, make sure:

1. `rozed` is running (`cargo run -p rozed-core` or `./rozed` in your project folder).
2. **Allow HTTP Requests** is enabled in Studio: **Home → Game Settings → Security**.

Clicking **Connect** sends a test request to `/status`. If the server responds, polling starts. The plugin then polls `/events/poll` every 500 ms and applies any queued events.

Clicking **Disconnect** stops the polling loop immediately.

---

## Push Flow (Zed → Studio)

When you save a `.luau` file in Zed:

1. The rozed watcher detects the change.
2. It reads the file, resolves the Roblox path from your mappings, and queues a `script-pushed` event.
3. The next time the plugin polls, it receives the event and calls `instance.Source = source` on the matching script in the data model.
4. If the script doesn't exist yet, it is created at the correct path with the correct class (`ModuleScript`, `Script`, or `LocalScript`).
5. The log shows `[PUSH] scriptName -> Roblox/Path`.

---

## Pull Flow (Studio → Disk)

Clicking **Pull from Zed**:

1. The plugin reads every mapped Roblox path from the server's `/status` response.
2. It walks all scripts under those paths and sends them to the server via `POST /pull`.
3. The server compares each script's source to the local file on disk.
4. The plugin polls and receives a `pull-ready` event with a list of files and whether each has a conflict.
5. For each conflicting file, a dialog appears — **Overwrite** replaces the local file, **Skip** leaves it.
6. Accepted files are sent via `POST /pull/confirm`. The server writes them to disk, keeping a `.backup` copy of the original.

---

## Log Colors

| Color | Meaning |
| --- | --- |
| Pink/red | `[ERROR]` — something failed |
| Green | `[SUCCESS]` — operation completed |
| Grey | `[INFO]`, `[PUSH]`, `[PULL]`, `[CONNECTED]`, `[INIT]`, `[CONFLICT]` — status messages |

---

## Changing the Port

The plugin hardcodes `local PORT = 5500` at the top of `rozed.luau`. If you change `port` in `rozed.toml`, update this line to match.

---

## Troubleshooting

> `[ERROR] Could not reach rozed on port 5500. Is it running?`

The server isn't running or is on a different port. Start `rozed` from your project directory, or check that `PORT` in the plugin matches `port` in `rozed.toml`.

> `[ERROR] Http requests are not enabled. Enable via game settings`

Go to **Home → Game Settings → Security** and enable **Allow HTTP Requests**.

> `[ERROR] Lost connection: ...`

The server stopped while the plugin was polling. Restart the binary and click **Connect** again.

> `[CONFLICT] filename differs — awaiting confirmation`

The local file and the Roblox version have different content. The dialog lets you choose which one wins. The other version is preserved as `.backup`.
