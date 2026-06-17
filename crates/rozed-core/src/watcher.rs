use std::collections::HashMap;
use std::path::PathBuf;
use anyhow::Result;
use notify::{
    Config as NotifyConfig, Event as NotifyEvent, EventKind,
    RecommendedWatcher, RecursiveMode, Watcher,
};
use tokio::sync::broadcast;
use crate::events::Event;
use crate::mapping::resolve_path;

pub async fn start_watcher(
    project_root: PathBuf,
    mappings: HashMap<String, String>,
    tx: broadcast::Sender<Event>,
) -> Result<()> {
    let (ntx, nrx) = std::sync::mpsc::channel();
    let mut watcher = RecommendedWatcher::new(ntx, NotifyConfig::default())?;

    for local_prefix in mappings.keys() {
        let watch_path = project_root.join(local_prefix);
        if watch_path.exists() {
            watcher.watch(&watch_path, RecursiveMode::Recursive)?;
        }
    }

    let ignore_file = project_root.join(".rozedignore");
    let (ignore_matcher, _) = ignore::gitignore::Gitignore::new(&ignore_file);

    // Use a plain OS thread so the loop is decoupled from the tokio runtime lifecycle.
    std::thread::spawn(move || {
        let _watcher = watcher;
        for result in nrx {
            if let Ok(NotifyEvent {
                kind: EventKind::Modify(_) | EventKind::Create(_),
                paths,
                ..
            }) = result
            {
                for path in paths {
                    if let Ok(rel) = path.strip_prefix(&project_root) {
                        if ignore_matcher.matched(rel, false).is_ignore() {
                            continue;
                        }
                        if let Some(script) = resolve_path(rel, &mappings) {
                            let source = std::fs::read_to_string(&path).unwrap_or_default();
                            println!("[PUSH] {} -> {}", script.name, script.roblox_path);
                            tx.send(Event::ScriptPushed {
                                name: script.name,
                                kind: script.kind.as_str().to_string(),
                                roblox_path: script.roblox_path,
                                source,
                            }).ok();
                        }
                    }
                }
            }
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    use std::collections::HashMap;
    use tokio::sync::broadcast;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_file_save_broadcasts_script_pushed() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("src/shared");
        fs::create_dir_all(&src).unwrap();

        let mut mappings = HashMap::new();
        mappings.insert("src/shared".into(), "ReplicatedStorage/Shared".into());

        let (tx, mut rx) = broadcast::channel(10);
        let root = dir.path().to_path_buf();
        let m = mappings.clone();

        tokio::spawn(async move {
            start_watcher(root, m, tx).await.unwrap();
        });

        sleep(Duration::from_millis(300)).await;
        fs::write(src.join("combat.module.luau"), "return {}").unwrap();
        sleep(Duration::from_millis(600)).await;

        let event = rx.try_recv().expect("expected a script-pushed event");
        match event {
            crate::events::Event::ScriptPushed { name, kind, .. } => {
                assert_eq!(name, "combat");
                assert_eq!(kind, "ModuleScript");
            }
            _ => panic!("unexpected event type"),
        }
    }

    #[tokio::test]
    async fn test_rozedignore_suppresses_event() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("src/shared");
        fs::create_dir_all(&src).unwrap();
        fs::write(dir.path().join(".rozedignore"), "src/shared/ignored.module.luau").unwrap();

        let mut mappings = HashMap::new();
        mappings.insert("src/shared".into(), "ReplicatedStorage/Shared".into());

        let (tx, mut rx) = broadcast::channel(10);
        let root = dir.path().to_path_buf();
        let m = mappings.clone();

        tokio::spawn(async move {
            start_watcher(root, m, tx).await.unwrap();
        });

        sleep(Duration::from_millis(300)).await;
        fs::write(src.join("ignored.module.luau"), "return {}").unwrap();
        sleep(Duration::from_millis(600)).await;

        assert!(rx.try_recv().is_err(), "ignored file should not trigger event");
    }
}
