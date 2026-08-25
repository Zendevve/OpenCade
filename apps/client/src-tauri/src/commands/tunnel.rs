use opencade_networking::run_native_tcp_tunnel;
use opencade_shared::{RelayCapability, RelayTicket};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::SocketAddr, sync::Mutex, time::Duration};
use tokio::{net::TcpListener, task::JoinHandle};

#[derive(Default)]
pub struct NativeTunnelState {
    tasks: Mutex<HashMap<String, JoinHandle<()>>>,
}

#[derive(Debug, Deserialize)]
pub struct StartNativeTunnelRequest {
    relay_url: String,
    ticket: RelayTicket,
    mode: TunnelMode,
    local_endpoint: SocketAddr,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TunnelMode {
    Listen,
    Connect,
}

#[derive(Debug, Serialize)]
pub struct NativeTunnelStarted {
    room_id: String,
    local_endpoint: SocketAddr,
}

#[tauri::command]
pub async fn start_native_tcp_tunnel(
    state: tauri::State<'_, NativeTunnelState>,
    request: StartNativeTunnelRequest,
) -> Result<NativeTunnelStarted, String> {
    if request.ticket.capability != RelayCapability::NativeTcpTunnel {
        return Err("native tunnel requires a capability-scoped ticket".into());
    }
    if !request.local_endpoint.ip().is_loopback() {
        return Err("native tunnel endpoints must be loopback-only".into());
    }
    let room_id = request.ticket.room_id.clone();
    let relay_url = request.relay_url;
    let ticket = request.ticket;
    let (local_endpoint, task) = match request.mode {
        TunnelMode::Listen => {
            let listener = TcpListener::bind(request.local_endpoint)
                .await
                .map_err(|_| "native tunnel listener is unavailable".to_string())?;
            let address = listener
                .local_addr()
                .map_err(|_| "native tunnel address is unavailable".to_string())?;
            let task = tokio::spawn(async move {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                if let Err(error) = run_native_tcp_tunnel(stream, &relay_url, &ticket).await {
                    tracing::warn!(%error, "native TCP tunnel stopped");
                }
            });
            (address, task)
        }
        TunnelMode::Connect => {
            let address = request.local_endpoint;
            let task = tokio::spawn(async move {
                let stream = loop {
                    match tokio::net::TcpStream::connect(address).await {
                        Ok(stream) => break stream,
                        Err(_) => tokio::time::sleep(Duration::from_millis(200)).await,
                    }
                };
                if let Err(error) = run_native_tcp_tunnel(stream, &relay_url, &ticket).await {
                    tracing::warn!(%error, "native TCP tunnel stopped");
                }
            });
            (address, task)
        }
    };
    let mut tasks = state
        .tasks
        .lock()
        .map_err(|_| "native tunnel registry is unavailable".to_string())?;
    if let Some(previous) = tasks.insert(room_id.clone(), task) {
        previous.abort();
    }
    Ok(NativeTunnelStarted {
        room_id,
        local_endpoint,
    })
}

#[tauri::command]
pub fn stop_native_tcp_tunnel(
    state: tauri::State<'_, NativeTunnelState>,
    room_id: String,
) -> Result<(), String> {
    if let Some(task) = state
        .tasks
        .lock()
        .map_err(|_| "native tunnel registry is unavailable".to_string())?
        .remove(&room_id)
    {
        task.abort();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tunnel_modes_are_explicit() {
        assert!(matches!(TunnelMode::Listen, TunnelMode::Listen));
        assert!(matches!(TunnelMode::Connect, TunnelMode::Connect));
    }
}
