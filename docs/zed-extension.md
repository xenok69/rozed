# Zed Extension

The Rozed Zed extension lives in `crates/rozed-zed/`. It is a WASM extension built with `zed_extension_api = "0.7"`. Its job is to bundle the rozed binary and the Roblox plugin so they can be distributed and installed directly from within Zed.

---

## What It Does

- Checks whether the workspace contains a `rozed.toml` before activating.
- Extracts the `rozed` binary from `assets/` and installs it to `%APPDATA%\rozed\rozed.exe` (Windows) or the equivalent on other platforms.
- Copies `assets/rozed.luau` into the Roblox plugins folder if it isn't already there.
- Exposes the binary as a language server command so Zed can manage its lifecycle.

---

## Installing as a Dev Extension

1. Open Zed.
2. Open the command palette (`Ctrl+Shift+P` / `Cmd+Shift+P`).
3. Run **Extensions: Install Dev Extension**.
4. Point Zed at the `crates/rozed-zed/` folder.

Zed compiles the WASM extension and installs it. You'll see "Rozed" in your installed extensions list.

---

## Current Limitations

### Manual server start required

The extension uses Zed's `language_server_command` hook to spawn the binary. This hook only fires when Zed opens a file that is associated with a registered language server. Because rozed is not a real LSP and the extension does not currently register a language association, the binary **does not auto-start**.

**Workaround:** start the server manually from a terminal in your project folder:

```bash
cargo run -p rozed-core
# or, after building:
./target/release/rozed
```

Auto-launch on workspace open is planned for a future release.

### Plugin copy is one-time

The extension only copies `rozed.luau` to the plugins folder if the file doesn't already exist. To update the plugin after an extension upgrade, delete the old `rozed.luau` from the plugins folder and reload the extension (or restart Zed).

---

## Building the Extension

```bash
cargo build -p rozed-zed --target wasm32-wasip1 --release
```

The compiled `.wasm` file lands at `target/wasm32-wasip1/release/rozed_zed.wasm`. Zed loads it from there when running as a dev extension.

---

## Extension Structure

```
crates/rozed-zed/
├── Cargo.toml          zed_extension_api = "0.7", crate-type = ["cdylib"]
├── extension.toml      id, name, schema_version = 1
├── src/
│   └── lib.rs          RozedExtension impl
└── assets/
    ├── rozed.exe       bundled binary (Windows, added at release time)
    └── rozed.luau      bundled plugin copy (synced from roblox-plugin/)
```

The `assets/` folder is not committed — binaries are attached at release time. During development, the binary is built locally and referenced directly.
