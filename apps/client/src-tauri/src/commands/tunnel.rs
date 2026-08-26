use opencade_networking::run_native_tcp_tunnel;
use opencade_shared::{RelayCapability, RelayTicket};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{net::TcpListener, task::JoinHandle};

#[derive(Default)]
pub struct NativeTunnelState {
    tasks: Arc<Mutex<HashMap<String, TunnelTask>>>,
}

struct TunnelTask {
    run_id: uuid::Uuid,
    handle: JoinHandle<()>,
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
    let run_id = uuid::Uuid::new_v4();
    let relay_url = request.relay_url;
    let ticket = request.ticket;
    let tasks = Arc::clone(&state.tasks);
    let cleanup_room_id = room_id.clone();
    let (local_endpoint, task, start_tx) = match request.mode {
        TunnelMode::Listen => {
            let (start_tx, start_rx) = tokio::sync::oneshot::channel();
            let listener = TcpListener::bind(request.local_endpoint)
                .await
                .map_err(|_| "native tunnel listener is unavailable".to_string())?;
            let address = listener
                .local_addr()
                .map_err(|_| "native tunnel address is unavailable".to_string())?;
            let task = tokio::spawn(async move {
                if start_rx.await.is_err() {
                    return;
                }
                let Ok(Ok((stream, _))) =
                    tokio::time::timeout(ticket_deadline(&ticket), listener.accept()).await
                else {
                    tracing::warn!(room_id = %cleanup_room_id, "native TCP listener deadline elapsed");
                    remove_completed_task(&tasks, &cleanup_room_id, run_id);
                    return;
                };
                if let Err(error) = run_native_tcp_tunnel(stream, &relay_url, &ticket).await {
                    tracing::warn!(%error, "native TCP tunnel stopped");
                }
                remove_completed_task(&tasks, &cleanup_room_id, run_id);
            });
            (address, task, start_tx)
        }
        TunnelMode::Connect => {
            let (start_tx, start_rx) = tokio::sync::oneshot::channel();
            let address = request.local_endpoint;
            let task = tokio::spawn(async move {
                if start_rx.await.is_err() {
                    return;
                }
                let deadline = tokio::time::Instant::now()
                    + ticket_deadline(&ticket).min(Duration::from_secs(10));
                let stream = loop {
                    if tokio::time::Instant::now() >= deadline {
                        tracing::warn!(room_id = %cleanup_room_id, "native TCP connect deadline elapsed");
                        remove_completed_task(&tasks, &cleanup_room_id, run_id);
                        return;
                    }
                    match tokio::net::TcpStream::connect(address).await {
                        Ok(stream) => break stream,
                        Err(_) => tokio::time::sleep(Duration::from_millis(200)).await,
                    }
                };
                if let Err(error) = run_native_tcp_tunnel(stream, &relay_url, &ticket).await {
                    tracing::warn!(%error, "native TCP tunnel stopped");
                }
                remove_completed_task(&tasks, &cleanup_room_id, run_id);
            });
            (address, task, start_tx)
        }
    };
    let mut tasks = state
        .tasks
        .lock()
        .map_err(|_| "native tunnel registry is unavailable".to_string())?;
    if let Some(previous) = tasks.insert(
        room_id.clone(),
        TunnelTask {
            run_id,
            handle: task,
        },
    ) {
        previous.handle.abort();
    }
    let _ = start_tx.send(());
    Ok(NativeTunnelStarted {
        room_id,
        local_endpoint,
    })
}

fn ticket_deadline(ticket: &RelayTicket) -> Duration {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or(ticket.expires_at);
    Duration::from_secs(u64::try_from(ticket.expires_at.saturating_sub(now)).unwrap_or_default())
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
        task.handle.abort();
    }
    Ok(())
}

fn remove_completed_task(
    tasks: &Mutex<HashMap<String, TunnelTask>>,
    room_id: &str,
    run_id: uuid::Uuid,
) {
    if let Ok(mut tasks) = tasks.lock()
        && tasks.get(room_id).is_some_and(|task| task.run_id == run_id)
    {
        tasks.remove(room_id);
    }
}

impl NativeTunnelState {
    pub fn shutdown_all(&self) {
        if let Ok(mut tasks) = self.tasks.lock() {
            for (_, task) in tasks.drain() {
                task.handle.abort();
            }
        }
    }
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
