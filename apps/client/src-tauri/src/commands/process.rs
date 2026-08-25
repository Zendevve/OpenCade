use opencade_adapter_fbneo::FbneoAdapter;
use opencade_adapter_retroarch::{CompatibilityFingerprint, RetroarchAdapter};
use opencade_emulator_sdk::{EmulatorAdapter, MatchDescriptor, PeerRole, TransportKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{Emitter, Manager};

const RETROARCH_ALPHA_PORT: u16 = 55_435;

#[derive(Default)]
pub struct ProcessState {
    children: Mutex<HashMap<u32, Arc<Mutex<Child>>>>,
}

pub fn fbneo_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .resource_dir()
        .map(|root| root.join("emulator").join("fbneo"))
        .map_err(|_| "application resource directory is unavailable".into())
}

fn retroarch_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    if let Some(root) = std::env::var_os("OPENCADE_RETROARCH_ROOT") {
        let root = PathBuf::from(root);
        if !root.is_absolute() {
            return Err("OPENCADE_RETROARCH_ROOT must be an absolute path".into());
        }
        return Ok(root);
    }
    app.path()
        .resource_dir()
        .map(|root| root.join("emulator").join("retroarch"))
        .map_err(|_| "application resource directory is unavailable".into())
}

fn validate_game_id(game_id: &str) -> Result<(), String> {
    if game_id.len() < 3
        || game_id.len() > 20
        || !game_id
            .chars()
            .all(|value| value.is_ascii_lowercase() || value.is_ascii_digit() || value == '_')
    {
        return Err("invalid game id".into());
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct RetroarchMatchRequest {
    api_url: String,
    session_token: String,
    launch_grant: String,
}

#[derive(Debug, Deserialize)]
struct AuthorizedLaunch {
    room_id: String,
    game_id: String,
    local_user_id: String,
    peer_user_id: String,
    role: PeerRole,
    local_endpoint: SocketAddr,
    peer_endpoint: SocketAddr,
    input_delay_frames: u8,
}

#[derive(Debug, Deserialize)]
struct ApiEnvelope<T> {
    payload: T,
}

#[derive(Debug, Serialize)]
struct ConsumeLaunchGrant<'a> {
    grant: &'a str,
}

#[derive(Debug, Serialize)]
pub struct RetroarchMatchLaunch {
    pid: u32,
    adapter: &'static str,
    room_id: String,
    fingerprint: CompatibilityFingerprint,
}

#[derive(Debug, Clone, Serialize)]
struct EmulatorExitEvent {
    pid: u32,
    room_id: Option<String>,
    exit_code: Option<i32>,
    success: bool,
}

struct NativeExitCallback {
    api_url: String,
    session_token: String,
    room_id: String,
}

#[derive(Serialize)]
struct FinishRoomBody {
    exit_code: Option<i32>,
}

#[tauri::command]
pub fn launch_game(
    app: tauri::AppHandle,
    state: tauri::State<'_, ProcessState>,
    game_id: String,
) -> Result<u32, String> {
    validate_game_id(&game_id)?;
    let root = fbneo_root(&app)?;
    let adapter = FbneoAdapter::new(&root);
    let child = adapter
        .launch(&root.join("ROMs").join(format!("{game_id}.zip")))
        .map_err(|error| error.to_string())?;
    register_child(&app, &state, child, None)
}

#[tauri::command]
pub async fn launch_retroarch_match(
    app: tauri::AppHandle,
    state: tauri::State<'_, ProcessState>,
    request: RetroarchMatchRequest,
) -> Result<RetroarchMatchLaunch, String> {
    let authorized = consume_launch_grant(&request).await?;
    validate_game_id(&authorized.game_id)?;
    let root = retroarch_root(&app)?;
    let adapter = RetroarchAdapter::new(&root);
    let rom = root
        .join("ROMs")
        .join(format!("{}.zip", authorized.game_id));
    let descriptor = MatchDescriptor {
        room_id: authorized.room_id.clone(),
        game_id: authorized.game_id,
        local_user_id: authorized.local_user_id,
        peer_user_id: authorized.peer_user_id,
        role: authorized.role,
        transport: TransportKind::DirectUdp,
        local_endpoint: authorized.local_endpoint,
        peer_endpoint: authorized.peer_endpoint,
        input_delay_frames: authorized.input_delay_frames,
    };
    descriptor
        .transport_lease()
        .map_err(|error| error.to_string())?;
    validate_native_endpoints(&descriptor)?;
    if descriptor.role == PeerRole::Host {
        ensure_native_port_available(descriptor.local_endpoint)?;
    }
    let (fingerprint, child) = tokio::task::spawn_blocking(move || {
        let fingerprint = adapter.fingerprint(&rom)?;
        let child = adapter.launch_match(&rom, &descriptor)?;
        Ok::<_, opencade_emulator_sdk::AdapterError>((fingerprint, child))
    })
    .await
    .map_err(|_| "native launch worker failed".to_string())?
    .map_err(|error| error.to_string())?;
    let room_id = authorized.room_id;
    let callback = NativeExitCallback {
        api_url: request.api_url,
        session_token: request.session_token,
        room_id: room_id.clone(),
    };
    let pid = register_child(&app, &state, child, Some(callback))?;
    Ok(RetroarchMatchLaunch {
        pid,
        adapter: "retroarch_fbneo",
        room_id,
        fingerprint,
    })
}

#[tauri::command]
pub fn stop_game(state: tauri::State<'_, ProcessState>, pid: u32) -> Result<(), String> {
    let child = state
        .children
        .lock()
        .map_err(|_| "process registry unavailable".to_string())?
        .get(&pid)
        .cloned()
        .ok_or_else(|| "emulator process not found".to_string())?;
    child
        .lock()
        .map_err(|_| "emulator process unavailable".to_string())?
        .kill()
        .map_err(|error| error.to_string())?;
    Ok(())
}

async fn consume_launch_grant(request: &RetroarchMatchRequest) -> Result<AuthorizedLaunch, String> {
    let base = reqwest::Url::parse(&request.api_url).map_err(|_| "OpenCade API URL is invalid")?;
    if !matches!(base.scheme(), "http" | "https")
        || !base.username().is_empty()
        || base.password().is_some()
        || base.query().is_some()
        || base.fragment().is_some()
    {
        return Err("OpenCade API URL is not allowed".into());
    }
    if base.scheme() == "http"
        && !matches!(base.host_str(), Some("localhost" | "127.0.0.1" | "::1"))
    {
        return Err("remote OpenCade API URLs must use HTTPS".into());
    }
    if request.session_token.is_empty() || request.launch_grant.is_empty() {
        return Err("authenticated launch grant is required".into());
    }
    let endpoint = format!(
        "{}/api/v1/match-launch-grants/consume",
        request.api_url.trim_end_matches('/')
    );
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| "launch authorization client unavailable")?;
    let response = client
        .post(endpoint)
        .bearer_auth(&request.session_token)
        .json(&ConsumeLaunchGrant {
            grant: &request.launch_grant,
        })
        .send()
        .await
        .map_err(|_| "launch authorization server is unreachable")?;
    if !response.status().is_success() {
        return Err("launch grant was rejected by the server".into());
    }
    response
        .json::<ApiEnvelope<AuthorizedLaunch>>()
        .await
        .map(|envelope| envelope.payload)
        .map_err(|_| "launch authorization response is invalid".into())
}

fn ensure_native_port_available(endpoint: SocketAddr) -> Result<(), String> {
    let bind_ip = if endpoint.is_ipv4() {
        std::net::Ipv4Addr::UNSPECIFIED.into()
    } else {
        std::net::Ipv6Addr::UNSPECIFIED.into()
    };
    std::net::TcpListener::bind(SocketAddr::new(bind_ip, endpoint.port()))
        .map(drop)
        .map_err(|_| format!("native netplay TCP port {} is unavailable", endpoint.port()))
}

fn validate_native_endpoints(descriptor: &MatchDescriptor) -> Result<(), String> {
    for endpoint in [descriptor.local_endpoint, descriptor.peer_endpoint] {
        if endpoint.port() != RETROARCH_ALPHA_PORT
            || endpoint.ip().is_unspecified()
            || endpoint.ip().is_multicast()
        {
            return Err("native alpha endpoints must be unicast on TCP port 55435".into());
        }
    }
    Ok(())
}

fn register_child(
    app: &tauri::AppHandle,
    state: &ProcessState,
    mut child: Child,
    exit_callback: Option<NativeExitCallback>,
) -> Result<u32, String> {
    if let Some(stdout) = child.stdout.take() {
        drain_process_output(stdout);
    }
    if let Some(stderr) = child.stderr.take() {
        drain_process_output(stderr);
    }
    let pid = child.id();
    let child = Arc::new(Mutex::new(child));
    state
        .children
        .lock()
        .map_err(|_| "process registry unavailable".to_string())?
        .insert(pid, Arc::clone(&child));

    let app = app.clone();
    std::thread::spawn(move || {
        let status = loop {
            let result = child
                .lock()
                .map_err(|_| ())
                .and_then(|mut child| child.try_wait().map_err(|_| ()));
            match result {
                Ok(Some(status)) => break Some(status),
                Ok(None) => std::thread::sleep(Duration::from_millis(200)),
                Err(()) => break None,
            }
        };
        if let Ok(mut children) = app.state::<ProcessState>().children.lock()
            && children
                .get(&pid)
                .is_some_and(|registered| Arc::ptr_eq(registered, &child))
        {
            children.remove(&pid);
        }
        let exit_code = status.as_ref().and_then(std::process::ExitStatus::code);
        let room_id = exit_callback
            .as_ref()
            .map(|callback| callback.room_id.clone());
        if let Some(callback) = exit_callback {
            report_native_exit(&callback, exit_code);
        }
        let event = EmulatorExitEvent {
            pid,
            room_id,
            exit_code,
            success: status.is_some_and(|status| status.success()),
        };
        if let Err(error) = app.emit("opencade://emulator-exited", event) {
            tracing::warn!(%error, pid, "failed to emit emulator exit event");
        }
    });
    Ok(pid)
}

fn report_native_exit(callback: &NativeExitCallback, exit_code: Option<i32>) {
    let endpoint = format!(
        "{}/api/v1/rooms/{}/finish",
        callback.api_url.trim_end_matches('/'),
        callback.room_id
    );
    let Ok(client) = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
    else {
        tracing::warn!(room_id = %callback.room_id, "native exit reporter unavailable");
        return;
    };
    for attempt in 1..=3 {
        let result = client
            .post(&endpoint)
            .bearer_auth(&callback.session_token)
            .json(&FinishRoomBody { exit_code })
            .send();
        if result.is_ok_and(|response| response.status().is_success()) {
            return;
        }
        if attempt < 3 {
            std::thread::sleep(Duration::from_millis(200 * attempt));
        }
    }
    tracing::warn!(room_id = %callback.room_id, "failed to report native process exit");
}

fn drain_process_output<R: Read + Send + 'static>(mut reader: R) {
    std::thread::spawn(move || {
        let _ = std::io::copy(&mut reader, &mut std::io::sink());
    });
}
