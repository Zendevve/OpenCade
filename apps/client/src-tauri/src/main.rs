#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

fn main() {
    tracing_subscriber::fmt::init();
    tauri::Builder::default()
        .manage(commands::process::ProcessState::default())
        .manage(commands::match_probe::MatchProbeState::default())
        .invoke_handler(tauri::generate_handler![
            commands::fs::scan_game,
            commands::process::launch_game,
            commands::process::stop_game,
            commands::diag::network_test,
            commands::match_probe::reserve_match_probe,
            commands::match_probe::run_reserved_match_probe,
            commands::match_probe::cancel_match_probe,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
