# rozed.toml — Configuration Reference

`rozed.toml` is the project config file. It must live in the **root of the folder you open in Zed** — the same folder you run the `rozed` binary from. The binary looks for it in the current working directory.

---

## Full Example

```toml
port = 5500
push_on_save = true
sync_interval_ms = 1000

[mappings]
"src/shared"  = "ReplicatedStorage/Shared"
"src/server"  = "ServerScriptService"
"src/client"  = "StarterPlayerScripts"
"src/ui"      = "ReplicatedStorage/UI"
```

---

## Keys

### `port`
| Type | Default |
| --- | --- |
| integer | `5500` |

Port the HTTP server listens on. The Roblox plugin is hardcoded to the same port — if you change this, update `local PORT = 5500` at the top of `rozed.luau` to match.

```toml
port = 5500
```

---

### `push_on_save`
| Type | Default |
| --- | --- |
| boolean | `true` |

When `true`, the file watcher is active. Saving any mapped `.luau` file automatically pushes it to Studio. Set to `false` if you only want to use the Pull flow (Studio → disk) and never push automatically.

```toml
push_on_save = true
```

---

### `sync_interval_ms`
| Type | Default |
| --- | --- |
| integer | `1000` |

Milliseconds between internal heartbeat ticks. This does not affect push speed (which is event-driven) — it controls background maintenance tasks. You can leave this at the default.

```toml
sync_interval_ms = 1000
```

---

### `[mappings]`

A table of `"local/path" = "Roblox/Path"` pairs.

- **Left side (local):** path relative to the folder containing `rozed.toml`
- **Right side (Roblox):** full path from `game` in Roblox's data model

```toml
[mappings]
"src/shared"  = "ReplicatedStorage/Shared"
"src/server"  = "ServerScriptService"
"src/client"  = "StarterPlayerScripts"
```

Any file outside a mapped folder is silently ignored by the watcher and by pull. You can have as many mappings as you need.

#### How paths resolve

Given this mapping:

```toml
"src/shared" = "ReplicatedStorage/Shared"
```

| Local file | Roblox instance |
| --- | --- |
| `src/shared/math.module.luau` | `ReplicatedStorage/Shared/math` (ModuleScript) |
| `src/shared/core/util.module.luau` | `ReplicatedStorage/Shared/core/util` (ModuleScript) |
| `src/shared/init.server.luau` | `ReplicatedStorage/Shared/init` (Script) |

Intermediate folders (`core/` in the example above) are created as `Folder` instances in Roblox if they don't already exist.

---

## File Naming Conventions

The suffix before `.luau` determines the Roblox script class:

| Suffix | Roblox class |
| --- | --- |
| `.module.luau` | `ModuleScript` |
| `.server.luau` | `Script` |
| `.client.luau` | `LocalScript` |

Files that don't match any of these suffixes are ignored.

---

## Notes

- `rozed.toml` should **not** be committed to source control if it contains team-specific port numbers or local paths that differ per developer. Add it to `.gitignore`.
- The file is read once on startup. Changing it requires a server restart.
