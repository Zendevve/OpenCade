use opencade_adapter_fbneo::FbneoAdapter;
use opencade_emulator_sdk::EmulatorAdapter;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Child;
use std::sync::Mutex;
use tauri::Manager;

#[derive(Default)]
pub struct ProcessState {
    children: Mutex<HashMap<u32, Child>>,
}

pub fn fbneo_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .resource_dir()
        .map(|root| root.join("emulator").join("fbneo"))
        .map_err(|_| "application resource directory is unavailable".into())
}

#[tauri::command]
pub fn launch_game(
    app: tauri::AppHandle,
    state: tauri::State<'_, ProcessState>,
    game_id: String,
) -> Result<u32, String> {
    if game_id.len() < 3
        || game_id.len() > 20
        || !game_id
            .chars()
            .all(|value| value.is_ascii_lowercase() || value.is_ascii_digit() || value == '_')
    {
        return Err("invalid game id".into());
    }
    let root = fbneo_root(&app)?;
    let adapter = FbneoAdapter::new(&root);
    let child = adapter
        .launch(&root.join("ROMs").join(format!("{game_id}.zip")))
        .map_err(|error| error.to_string())?;
    let pid = child.id();
    state
        .children
        .lock()
        .map_err(|_| "process registry unavailable".to_string())?
        .insert(pid, child);
    Ok(pid)
}

#[tauri::command]
pub fn stop_game(state: tauri::State<'_, ProcessState>, pid: u32) -> Result<(), String> {
    let mut child = state
        .children
        .lock()
        .map_err(|_| "process registry unavailable".to_string())?
        .remove(&pid)
        .ok_or_else(|| "emulator process not found".to_string())?;
    child.kill().map_err(|error| error.to_string())?;
    child.wait().map_err(|error| error.to_string())?;
    Ok(())
}
