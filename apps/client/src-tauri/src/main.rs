#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use tauri::Manager;

fn main() {
    tracing_subscriber::fmt::init();
    let runtime = commands::runtime::RuntimeConfig::from_env()
        .expect("invalid OpenCade runtime configuration");
    let app = tauri::Builder::default()
        .manage(runtime)
        .manage(commands::process::ProcessState::default())
        .manage(commands::match_probe::MatchProbeState::default())
        .manage(commands::tunnel::NativeTunnelState::default())
        .invoke_handler(tauri::generate_handler![
            commands::fs::scan_game,
            commands::process::launch_game,
            commands::process::launch_retroarch_match,
            commands::process::retroarch_preflight,
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
            commands::tunnel::start_native_tcp_tunnel,
            commands::tunnel::stop_native_tcp_tunnel,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");
    app.run(|handle, event| {
        if matches!(
            event,
            tauri::RunEvent::Exit | tauri::RunEvent::ExitRequested { .. }
        ) {
            handle
                .state::<commands::process::ProcessState>()
                .shutdown_all();
            handle
                .state::<commands::tunnel::NativeTunnelState>()
                .shutdown_all();
            handle
                .state::<commands::match_probe::MatchProbeState>()
                .shutdown_all();
        }
    });
}
