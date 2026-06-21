use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, SystemTime};
use tokio::sync::broadcast;
use crate::events::Event;
use crate::mapping::resolve_path;

pub fn push_changed_files(
    project_root: &Path,
    mappings: &HashMap<String, String>,
    tx: &broadcast::Sender<Event>,
    window: Duration,
) {
    let cutoff = SystemTime::now()
        .checked_sub(window)
        .unwrap_or(SystemTime::UNIX_EPOCH);
    push_files(project_root, mappings, tx, cutoff);
}

pub fn push_all_files(
    project_root: &Path,
    mappings: &HashMap<String, String>,
    tx: &broadcast::Sender<Event>,
) {
    push_files(project_root, mappings, tx, SystemTime::UNIX_EPOCH);
}

fn push_files(
    project_root: &Path,
    mappings: &HashMap<String, String>,
    tx: &broadcast::Sender<Event>,
    cutoff: SystemTime,
) {
    let (ignore_matcher, _) =
        ignore::gitignore::Gitignore::new(project_root.join(".rozedignore"));
    for local_prefix in mappings.keys() {
        let dir = project_root.join(local_prefix);
        if dir.exists() {
            walk_and_push(&dir, project_root, mappings, tx, cutoff, &ignore_matcher);
        }
    }
}

fn walk_and_push(
    dir: &Path,
    project_root: &Path,
    mappings: &HashMap<String, String>,
    tx: &broadcast::Sender<Event>,
    cutoff: SystemTime,
    ignore_matcher: &ignore::gitignore::Gitignore,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_and_push(&path, project_root, mappings, tx, cutoff, ignore_matcher);
        } else if let Ok(rel) = path.strip_prefix(project_root) {
            if ignore_matcher.matched(rel, false).is_ignore() {
                continue;
            }
            let mtime = std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            if mtime < cutoff {
                continue;
            }
            if let Some(script) = resolve_path(rel, mappings) {
                let source = std::fs::read_to_string(&path).unwrap_or_default();
                eprintln!("[SYNC] {} -> {}", script.name, script.roblox_path);
                tx.send(Event::ScriptPushed {
                    name: script.name,
                    kind: script.kind.as_str().to_string(),
                    roblox_path: script.roblox_path,
                    source,
                })
                .ok();
            }
        }
    }
}
