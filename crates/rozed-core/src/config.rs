use std::collections::HashMap;
use std::path::Path;
use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    sync_interval_ms: Option<u64>,
    push_on_save: Option<bool>,
    pub mappings: HashMap<String, String>,
}

impl Config {
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = project_root.join("rozed.toml");
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("rozed.toml not found at {}", path.display()))?;
        toml::from_str(&content).context("Failed to parse rozed.toml")
    }

    pub fn sync_interval_ms(&self) -> u64 { self.sync_interval_ms.unwrap_or(500) }
    pub fn push_on_save(&self) -> bool { self.push_on_save.unwrap_or(true) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_parse_full_config() {
        let dir = TempDir::new().unwrap();
        let toml = r#"
sync_interval_ms = 250
push_on_save = false

[mappings]
"src/shared" = "ReplicatedStorage/Shared"
"src/server" = "ServerScriptService"
"#;
        fs::write(dir.path().join("rozed.toml"), toml).unwrap();
        let config = Config::load(dir.path()).unwrap();
        assert_eq!(config.sync_interval_ms(), 250);
        assert!(!config.push_on_save());
        assert_eq!(
            config.mappings.get("src/shared").unwrap(),
            "ReplicatedStorage/Shared"
        );
    }

    #[test]
    fn test_defaults() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("rozed.toml"), "[mappings]\n").unwrap();
        let config = Config::load(dir.path()).unwrap();
        assert_eq!(config.sync_interval_ms(), 500);
        assert!(config.push_on_save());
    }

    #[test]
    fn test_missing_config_errors() {
        let dir = TempDir::new().unwrap();
        assert!(Config::load(dir.path()).is_err());
    }
}
