use openfight_adapter_fbneo::FbneoAdapter;
use openfight_emulator_sdk::EmulatorAdapter;
use serde::Serialize;
use std::path::PathBuf;

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
pub fn scan_game(app: tauri::AppHandle, game_id: String) -> Result<GameAvailability, String> {
    scan_fbneo_rom(super::process::fbneo_root(&app)?, &game_id)
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
