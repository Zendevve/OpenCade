use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GameDefError {
    #[error("io error reading {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("toml parse error in {path}: {source}")]
    Toml {
        path: String,
        source: toml::de::Error,
    },
    #[error("validation error in {path}: {message}")]
    Validation { path: String, message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameDefinition {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub emulator: String,
    #[serde(default)]
    pub launch: LaunchConfig,
    #[serde(default)]
    pub validation: ValidationConfig,
    #[serde(default)]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct LaunchConfig {
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ValidationConfig {
    #[serde(default)]
    pub required_files: Vec<String>,
    #[serde(default)]
    pub bios: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Metadata {
    #[serde(default)]
    pub year: Option<u32>,
    #[serde(default)]
    pub developer: Option<String>,
    #[serde(default)]
    pub players: Option<u32>,
}

impl GameDefinition {
    pub fn validate(&self, path: &str) -> Result<(), GameDefError> {
        if self.schema_version != 1 {
            return Err(GameDefError::Validation {
                path: path.to_string(),
                message: format!("unsupported schema_version {}, expected 1", self.schema_version),
            });
        }
        let id_re = Regex::new(r"^[a-z0-9_-]{3,20}$").expect("valid regex");
        if !id_re.is_match(&self.id) {
            return Err(GameDefError::Validation {
                path: path.to_string(),
                message: format!("id '{}' must match ^[a-z0-9_-]{{3,20}}$", self.id),
            });
        }
        if self.name.trim().is_empty() {
            return Err(GameDefError::Validation {
                path: path.to_string(),
                message: "name must not be empty".to_string(),
            });
        }
        let allowed_emulators = ["fbneo", "flycast", "snes9x"];
        if !allowed_emulators.contains(&self.emulator.as_str()) {
            return Err(GameDefError::Validation {
                path: path.to_string(),
                message: format!("emulator '{}' must be one of {:?}", self.emulator, allowed_emulators),
            });
        }
        if self.launch.args.is_empty() {
            return Err(GameDefError::Validation {
                path: path.to_string(),
                message: "launch.args must not be empty".to_string(),
            });
        }
        let has_rom_placeholder = self.launch.args.iter().any(|a| a.contains("{rom}"));
        if !has_rom_placeholder {
            return Err(GameDefError::Validation {
                path: path.to_string(),
                message: "launch.args must contain {rom} placeholder".to_string(),
            });
        }
        Ok(())
    }

    pub fn render_args(&self, rom_path: &Path) -> Vec<String> {
        let rom_str = rom_path.to_string_lossy().to_string();
        self.launch.args.iter().map(|a| a.replace("{rom}", &rom_str)).collect()
    }
}

pub fn load_from_str(content: &str, path: &str) -> Result<GameDefinition, GameDefError> {
    let def: GameDefinition = toml::from_str(content).map_err(|e| GameDefError::Toml {
        path: path.to_string(),
        source: e,
    })?;
    def.validate(path)?;
    Ok(def)
}

pub fn load_from_path(path: &Path) -> Result<GameDefinition, GameDefError> {
    let content = std::fs::read_to_string(path).map_err(|e| GameDefError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    load_from_str(&content, &path.display().to_string())
}

pub fn load_all_from_dir(dir: &Path) -> Result<Vec<GameDefinition>, GameDefError> {
    let mut out = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| GameDefError::Io {
        path: dir.display().to_string(),
        source: e,
    })?;
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| GameDefError::Io {
            path: dir.display().to_string(),
            source: e,
        })?;
        let p = entry.path();
        if p.extension().and_then(|s| s.to_str()) == Some("toml") {
            paths.push(p);
        }
    }
    paths.sort();
    for p in paths {
        let def = load_from_path(&p)?;
        out.push(def);
    }
    if out.is_empty() {
        return Err(GameDefError::Validation {
            path: dir.display().to_string(),
            message: "no .toml game definitions found".to_string(),
        });
    }
    let mut seen = std::collections::HashSet::new();
    for def in &out {
        if !seen.insert(def.id.clone()) {
            return Err(GameDefError::Validation {
                path: dir.display().to_string(),
                message: format!("duplicate game id '{}'", def.id),
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_toml(id: &str) -> String {
        format!(
            r#"
schema_version = 1
id = "{id}"
name = "Test Game {id}"
emulator = "fbneo"

[launch]
args = ["-rom", "{{rom}}", "-window"]

[validation]
required_files = ["{id}.zip", "neogeo.zip"]
bios = "neogeo.zip"

[metadata]
year = 1998
developer = "Test"
players = 2
"#
        )
    }

    #[test]
    fn load_valid() {
        let toml = sample_toml("kof98");
        let def = load_from_str(&toml, "kof98.toml").unwrap();
        assert_eq!(def.id, "kof98");
        assert_eq!(def.emulator, "fbneo");
        assert_eq!(def.validation.required_files, vec!["kof98.zip", "neogeo.zip"]);
        assert_eq!(def.launch.args, vec!["-rom", "{rom}", "-window"]);
    }

    #[test]
    fn rejects_unknown_schema() {
        let toml = sample_toml("kof98").replace("schema_version = 1", "schema_version = 2");
        let err = load_from_str(&toml, "kof98.toml").unwrap_err();
        assert!(err.to_string().contains("schema_version"));
    }

    #[test]
    fn rejects_bad_id() {
        let toml = sample_toml("BadID!");
        let err = load_from_str(&toml, "bad.toml").unwrap_err();
        assert!(err.to_string().contains("id"));
    }

    #[test]
    fn rejects_missing_rom_placeholder() {
        let toml = r#"
schema_version = 1
id = "test01"
name = "Test"
emulator = "fbneo"
[launch]
args = ["-window"]
"#;
        assert!(load_from_str(toml, "test.toml").is_err());
    }

    #[test]
    fn render_args_substitutes() {
        let def = load_from_str(&sample_toml("sfiii3"), "sfiii3.toml").unwrap();
        let out = def.render_args(Path::new("C:/ROMS/sfiii3.zip"));
        assert_eq!(out, vec!["-rom", "C:/ROMS/sfiii3.zip", "-window"]);
    }

    #[test]
    fn load_all_from_dir_sorted_and_unique() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("kof98.toml"), sample_toml("kof98")).unwrap();
        std::fs::write(dir.path().join("sfiii3.toml"), sample_toml("sfiii3")).unwrap();
        let all = load_all_from_dir(dir.path()).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].id, "kof98");
        assert_eq!(all[1].id, "sfiii3");
    }

    #[test]
    fn rejects_duplicate_ids() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.toml"), sample_toml("kof98")).unwrap();
        std::fs::write(dir.path().join("b.toml"), sample_toml("kof98")).unwrap();
        assert!(load_all_from_dir(dir.path()).is_err());
    }
}
