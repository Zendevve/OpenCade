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
            commands::process::launch_retroarch_match,
            commands::process::stop_game,
            commands::diag::network_test,
            commands::match_probe::reserve_match_probe,
            commands::match_probe::run_reserved_match_probe,
            commands::match_probe::run_relay_match_probe_command,
            commands::match_probe::cancel_match_probe,
            commands::runtime::runtime_config,
            commands::session::store_session_token,
            commands::session::load_session_token,
            commands::session::clear_session_token,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
