use std::collections::HashMap;
use std::path::{Path, PathBuf};
use anyhow::Result;
use crate::events::{IncomingScript, PullFile};

pub fn check_conflict(
    script: &IncomingScript,
    project_root: &Path,
    mappings: &HashMap<String, String>,
) -> PullFile {
    let local_path = resolve_to_local_path(script, project_root, mappings);
    let conflict = local_path.as_ref().map(|p| {
        if p.exists() {
            let existing = std::fs::read_to_string(p).unwrap_or_default();
            existing.trim() != script.source.trim()
        } else {
            false
        }
    }).unwrap_or(false);

    PullFile {
        roblox_path: script.roblox_path.clone(),
        name: script.name.clone(),
        kind: script.kind.clone(),
        source: script.source.clone(),
        conflict,
    }
}

pub fn write_script(
    file: &PullFile,
    project_root: &Path,
    mappings: &HashMap<String, String>,
) -> Result<()> {
    let script = IncomingScript {
        roblox_path: file.roblox_path.clone(),
        name: file.name.clone(),
        kind: file.kind.clone(),
        source: file.source.clone(),
    };
    let local_path = resolve_to_local_path(&script, project_root, mappings)
        .ok_or_else(|| anyhow::anyhow!("Cannot resolve: {}", file.roblox_path))?;

    if local_path.exists() {
        let backup_path = PathBuf::from(format!("{}.backup", local_path.display()));
        std::fs::copy(&local_path, &backup_path)?;
        eprintln!("[BACKUP] {}.backup created", local_path.display());
    }

    if let Some(parent) = local_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&local_path, &file.source)?;
    eprintln!("[SUCCESS] {} written", local_path.display());
    Ok(())
}

fn resolve_to_local_path(
    script: &IncomingScript,
    project_root: &Path,
    mappings: &HashMap<String, String>,
) -> Option<PathBuf> {
    let ext = match script.kind.as_str() {
        "ModuleScript" => "module.luau",
        "Script" => "server.luau",
        "LocalScript" => "client.luau",
        _ => return None,
    };
    for (local_prefix, roblox_prefix) in mappings {
        if let Some(rest) = script.roblox_path.strip_prefix(&format!("{}/", roblox_prefix)) {
            let filename = format!("{}.{}", rest, ext);
            return Some(project_root.join(local_prefix).join(filename));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    use std::collections::HashMap;

    fn mappings() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("src/shared".into(), "ReplicatedStorage/Shared".into());
        m
    }

    #[test]
    fn test_new_file_no_conflict() {
        let dir = TempDir::new().unwrap();
        let script = IncomingScript {
            roblox_path: "ReplicatedStorage/Shared/combat".into(),
            name: "combat".into(),
            kind: "ModuleScript".into(),
            source: "return {}".into(),
        };
        let result = check_conflict(&script, dir.path(), &mappings());
        assert!(!result.conflict);
    }

    #[test]
    fn test_identical_content_no_conflict() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("src/shared")).unwrap();
        fs::write(dir.path().join("src/shared/combat.module.luau"), "return {}").unwrap();
        let script = IncomingScript {
            roblox_path: "ReplicatedStorage/Shared/combat".into(),
            name: "combat".into(),
            kind: "ModuleScript".into(),
            source: "return {}".into(),
        };
        let result = check_conflict(&script, dir.path(), &mappings());
        assert!(!result.conflict);
    }

    #[test]
    fn test_different_content_is_conflict() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("src/shared")).unwrap();
        fs::write(dir.path().join("src/shared/combat.module.luau"), "return {old=true}").unwrap();
        let script = IncomingScript {
            roblox_path: "ReplicatedStorage/Shared/combat".into(),
            name: "combat".into(),
            kind: "ModuleScript".into(),
            source: "return {new=true}".into(),
        };
        let result = check_conflict(&script, dir.path(), &mappings());
        assert!(result.conflict);
    }

    #[test]
    fn test_write_creates_backup_and_overwrites() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("src/shared")).unwrap();
        let original = dir.path().join("src/shared/combat.module.luau");
        fs::write(&original, "return {old=true}").unwrap();

        let file = PullFile {
            roblox_path: "ReplicatedStorage/Shared/combat".into(),
            name: "combat".into(),
            kind: "ModuleScript".into(),
            source: "return {new=true}".into(),
            conflict: true,
        };
        write_script(&file, dir.path(), &mappings()).unwrap();

        assert_eq!(fs::read_to_string(&original).unwrap(), "return {new=true}");
        let backup = dir.path().join("src/shared/combat.module.luau.backup");
        assert!(backup.exists());
        assert_eq!(fs::read_to_string(backup).unwrap(), "return {old=true}");
    }
}
