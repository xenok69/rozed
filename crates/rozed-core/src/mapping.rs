use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub enum ScriptKind {
    ModuleScript,
    Script,
    LocalScript,
}

impl ScriptKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ScriptKind::ModuleScript => "ModuleScript",
            ScriptKind::Script => "Script",
            ScriptKind::LocalScript => "LocalScript",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedScript {
    pub name: String,
    pub kind: ScriptKind,
    pub roblox_path: String,
    pub local_path: PathBuf,
}

pub fn resolve_path(
    local_path: &Path,
    mappings: &HashMap<String, String>,
) -> Option<ResolvedScript> {
    let filename = local_path.file_name()?.to_str()?;
    let (name, kind) = parse_script_name(filename)?;

    let normalized = local_path.to_string_lossy().replace('\\', "/");

    for (local_prefix, roblox_prefix) in mappings {
        let prefix = local_prefix.trim_end_matches('/');
        if let Some(rest) = normalized.strip_prefix(&format!("{}/", prefix)) {
            let parent_rest = Path::new(rest)
                .parent()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            let roblox_path = if parent_rest.is_empty() {
                format!("{}/{}", roblox_prefix, name)
            } else {
                format!("{}/{}/{}", roblox_prefix, parent_rest, name)
            };
            return Some(ResolvedScript {
                name,
                kind,
                roblox_path,
                local_path: local_path.to_path_buf(),
            });
        }
    }
    None
}

fn parse_script_name(filename: &str) -> Option<(String, ScriptKind)> {
    let without_luau = filename.strip_suffix(".luau")?;
    if let Some(name) = without_luau.strip_suffix(".module") {
        Some((name.to_string(), ScriptKind::ModuleScript))
    } else if let Some(name) = without_luau.strip_suffix(".server") {
        Some((name.to_string(), ScriptKind::Script))
    } else if let Some(name) = without_luau.strip_suffix(".client") {
        Some((name.to_string(), ScriptKind::LocalScript))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn mappings() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("src/shared".into(), "ReplicatedStorage/Shared".into());
        m.insert("src/server".into(), "ServerScriptService".into());
        m
    }

    #[test]
    fn test_module_script() {
        let path = PathBuf::from("src/shared/combat.module.luau");
        let result = resolve_path(&path, &mappings()).unwrap();
        assert_eq!(result.name, "combat");
        assert!(matches!(result.kind, ScriptKind::ModuleScript));
        assert_eq!(result.roblox_path, "ReplicatedStorage/Shared/combat");
    }

    #[test]
    fn test_server_script() {
        let path = PathBuf::from("src/server/init.server.luau");
        let result = resolve_path(&path, &mappings()).unwrap();
        assert_eq!(result.name, "init");
        assert!(matches!(result.kind, ScriptKind::Script));
        assert_eq!(result.roblox_path, "ServerScriptService/init");
    }

    #[test]
    fn test_client_script() {
        let path = PathBuf::from("src/shared/loader.client.luau");
        let result = resolve_path(&path, &mappings()).unwrap();
        assert_eq!(result.name, "loader");
        assert!(matches!(result.kind, ScriptKind::LocalScript));
    }

    #[test]
    fn test_unmapped_path_returns_none() {
        let path = PathBuf::from("src/other/foo.module.luau");
        assert!(resolve_path(&path, &mappings()).is_none());
    }

    #[test]
    fn test_non_luau_suffix_returns_none() {
        let path = PathBuf::from("src/shared/readme.txt");
        assert!(resolve_path(&path, &mappings()).is_none());
    }
}
