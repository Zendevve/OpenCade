use opencade_adapter_fbneo::FbneoAdapter;
use opencade_adapter_retroarch::RetroarchAdapter;
use opencade_emulator_sdk::EmulatorAdapter;
use serde::Serialize;
use std::path::PathBuf;
use tauri::Manager;

fn retroarch_test_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    if let Some(root) = std::env::var_os("OPENCADE_RETROARCH_ROOT") {
        let p = PathBuf::from(root);
        if !p.is_absolute() {
            return Err("OPENCADE_RETROARCH_ROOT must be an absolute path".into());
        }
        return Ok(p);
    }
    let resource_root = app
        .path()
        .resource_dir()
        .map(|r| r.join("emulator").join("retroarch"))
        .map_err(|_| "application resource directory is unavailable".to_string())?;
    if resource_root.is_dir() {
        return Ok(resource_root);
    }
    let bases = [
        std::env::current_dir().ok(),
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")),
    ];
    for base in bases.into_iter().flatten() {
        let mut cur = base;
        for _ in 0..5 {
            let cand = cur.join("fixtures/libretro/opencade-test-core");
            if cand.is_dir() {
                return Ok(cand);
            }
            if let Some(parent) = cur.parent() {
                cur = parent.to_path_buf();
            } else {
                break;
            }
        }
    }
    Ok(resource_root)
}

#[derive(Debug, Serialize)]
pub struct GameAvailability {
    pub game_id: String,
    pub available: bool,
    pub warnings: Vec<String>,
}

pub fn scan_fbneo_rom(root: PathBuf, game_id: &str) -> Result<GameAvailability, String> {
    if game_id.len() < 3
        || game_id.len() > 20
        || !game_id
            .chars()
            .all(|value| value.is_ascii_lowercase() || value.is_ascii_digit() || value == '_')
    {
        return Err("invalid game id".into());
    }
    let adapter = FbneoAdapter::new(&root);
    adapter.detect(&root).map_err(|error| error.to_string())?;
    let rom = root.join("ROMs").join(format!("{game_id}.zip"));
    match adapter.validate(&rom) {
        Ok(report) => Ok(GameAvailability {
            game_id: game_id.into(),
            available: report.valid,
            warnings: report.warnings,
        }),
        Err(error) => Ok(GameAvailability {
            game_id: game_id.into(),
            available: false,
            warnings: vec![error.to_string()],
        }),
    }
}

#[tauri::command]
pub async fn scan_game(app: tauri::AppHandle, game_id: String) -> Result<GameAvailability, String> {
    if game_id == opencade_adapter_retroarch::TEST_GAME_ID {
        let root = retroarch_test_root(&app)?;
        return tokio::task::spawn_blocking(move || {
            let adapter = RetroarchAdapter::new(&root);
            let available = adapter.is_available_for_game(&game_id);
            if available {
                Ok(GameAvailability {
                    game_id,
                    available: true,
                    warnings: Vec::new(),
                })
            } else {
                Ok(GameAvailability {
                    game_id,
                    available: false,
                    warnings: vec![format!("test content not found below {}", root.display())],
                })
            }
        })
        .await
        .map_err(|error| format!("game scan worker failed: {error}"))?;
    }
    let root = super::process::fbneo_root(&app)?;
    tokio::task::spawn_blocking(move || scan_fbneo_rom(root, &game_id))
        .await
        .map_err(|error| format!("game scan worker failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_ids_that_could_become_paths() {
        assert!(scan_fbneo_rom(PathBuf::from("unused"), "../rom").is_err());
        assert!(scan_fbneo_rom(PathBuf::from("unused"), "SFIII3").is_err());
    }
}
