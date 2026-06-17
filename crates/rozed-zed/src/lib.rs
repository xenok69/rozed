use std::path::PathBuf;
use zed_extension_api as zed;

struct RozedExtension {
    binary_path: Option<PathBuf>,
}

impl RozedExtension {
    fn binary_name() -> &'static str {
        if cfg!(target_os = "windows") { "rozed.exe" } else { "rozed" }
    }

    fn install_binary(&mut self) -> zed::Result<PathBuf> {
        let name = Self::binary_name();
        let dest = PathBuf::from(format!(
            "{}/rozed/{}",
            std::env::var("APPDATA").unwrap_or_else(|_| ".".into()),
            name
        ));

        if dest.exists() {
            self.binary_path = Some(dest.clone());
            return Ok(dest);
        }

        let asset_path = format!("assets/{}", name);
        let bytes = std::fs::read(&asset_path)
            .map_err(|e| format!("could not read bundled binary at {}: {}", asset_path, e))?;

        std::fs::create_dir_all(dest.parent().unwrap())
            .map_err(|e| e.to_string())?;
        std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
        }

        self.binary_path = Some(dest.clone());
        Ok(dest)
    }

    fn install_roblox_plugin(&self) -> zed::Result<()> {
        #[cfg(target_os = "windows")]
        {
            if let Ok(local) = std::env::var("LOCALAPPDATA") {
                let plugins = PathBuf::from(local).join("Roblox").join("Plugins");
                let dest = plugins.join("rozed.luau");
                if plugins.exists() && !dest.exists() {
                    let bytes = std::fs::read("assets/rozed.luau")
                        .map_err(|e| e.to_string())?;
                    std::fs::write(&dest, bytes).map_err(|e| e.to_string())?;
                }
            }
        }
        Ok(())
    }
}

impl zed::Extension for RozedExtension {
    fn new() -> Self {
        RozedExtension { binary_path: None }
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        if worktree.read_text_file("rozed.toml").is_err() {
            return Err("No rozed.toml found -- not activating".into());
        }

        self.install_roblox_plugin()?;
        let binary = self.install_binary()?;

        Ok(zed::Command {
            command: binary.to_string_lossy().into_owned(),
            args: vec![],
            env: vec![],
        })
    }

}


zed::register_extension!(RozedExtension);
