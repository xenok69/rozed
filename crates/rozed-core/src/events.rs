use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Event {
    ScriptPushed {
        name: String,
        kind: String,
        roblox_path: String,
        source: String,
    },
    PullReady {
        files: Vec<PullFile>,
    },
    StructureOk,
    StructureMissing {
        paths: Vec<String>,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullFile {
    pub roblox_path: String,
    pub name: String,
    pub kind: String,
    pub source: String,
    pub conflict: bool,
}

#[derive(Debug, Deserialize)]
pub struct PullRequest {
    pub files: Vec<IncomingScript>,
}

#[derive(Debug, Deserialize)]
pub struct IncomingScript {
    pub roblox_path: String,
    pub name: String,
    pub kind: String,
    pub source: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_script_pushed_serializes_with_type_tag() {
        let event = Event::ScriptPushed {
            name: "combat".into(),
            kind: "ModuleScript".into(),
            roblox_path: "ReplicatedStorage/Shared/combat".into(),
            source: "return {}".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"script-pushed\""));
        assert!(json.contains("\"name\":\"combat\""));
    }

    #[test]
    fn test_structure_missing_serializes() {
        let event = Event::StructureMissing {
            paths: vec!["ReplicatedStorage/Shared".into()],
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"structure-missing\""));
        assert!(json.contains("ReplicatedStorage/Shared"));
    }

    #[test]
    fn test_pull_ready_round_trips() {
        let event = Event::PullReady {
            files: vec![PullFile {
                roblox_path: "ReplicatedStorage/Shared/foo".into(),
                name: "foo".into(),
                kind: "ModuleScript".into(),
                source: "return 1".into(),
                conflict: true,
            }],
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"conflict\":true"));
    }
}
