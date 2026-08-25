use futures_util::{SinkExt, StreamExt};
use opencade_shared::RelayTicket;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url;

use crate::TransportError;

pub const NATIVE_TUNNEL_FRAME_BYTES: usize = 16 * 1024;

/// Bridge one already-authorized local TCP stream over the signed two-peer WebSocket relay.
///
/// This is intentionally distinct from the deterministic UDP readiness probe. Callers must obtain
/// an explicit native-tunnel ticket and provide a loopback-side stream owned by the emulator
/// adapter. Frames are bounded and WebSocket backpressure is awaited rather than buffered without
/// limit.
pub async fn run_native_tcp_tunnel(
    local: TcpStream,
    relay_url: &str,
    ticket: &RelayTicket,
) -> Result<(), TransportError> {
    let mut url = Url::parse(relay_url)
        .map_err(|_| TransportError::Relay("native tunnel URL is invalid".into()))?;
    if !matches!(url.scheme(), "ws" | "wss") {
        return Err(TransportError::Relay(
            "native tunnel URL must use ws or wss".into(),
        ));
    }
    url.query_pairs_mut()
        .append_pair("room_id", &ticket.room_id)
        .append_pair("user_id", &ticket.user_id)
        .append_pair("expires_at", &ticket.expires_at.to_string())
        .append_pair("capability", ticket.capability.as_str())
        .append_pair("signature", &ticket.signature);
    let (websocket, _) = connect_async(url.as_str())
        .await
        .map_err(|_| TransportError::Relay("native tunnel connection failed".into()))?;
    let (mut ws_write, mut ws_read) = websocket.split();
    let (mut tcp_read, mut tcp_write) = local.into_split();
    let tcp_to_relay = async {
        let mut buffer = vec![0_u8; NATIVE_TUNNEL_FRAME_BYTES];
        loop {
            let count = tcp_read
                .read(&mut buffer)
                .await
                .map_err(|_| TransportError::Relay("native TCP read failed".into()))?;
            if count == 0 {
                ws_write
                    .send(Message::Close(None))
                    .await
                    .map_err(|_| TransportError::Relay("native tunnel close failed".into()))?;
                return Ok::<(), TransportError>(());
            }
            ws_write
                .send(Message::Binary(buffer[..count].to_vec().into()))
                .await
                .map_err(|_| TransportError::Relay("native tunnel send failed".into()))?;
        }
    };
    let relay_to_tcp = async {
        loop {
            match ws_read.next().await {
                Some(Ok(Message::Binary(payload)))
                    if payload.len() <= NATIVE_TUNNEL_FRAME_BYTES =>
                {
                    tcp_write
                        .write_all(&payload)
                        .await
                        .map_err(|_| TransportError::Relay("native TCP write failed".into()))?;
                }
                Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {}
                Some(Ok(Message::Close(_))) | None => return Ok::<(), TransportError>(()),
                Some(Ok(Message::Binary(_))) => {
                    return Err(TransportError::Relay(
                        "native tunnel frame exceeds 16 KiB".into(),
                    ));
                }
                Some(Ok(Message::Text(_))) | Some(Ok(Message::Frame(_))) => {
                    return Err(TransportError::Relay(
                        "native tunnel received an invalid frame".into(),
                    ));
                }
                Some(Err(_)) => {
                    return Err(TransportError::Relay("native tunnel receive failed".into()));
                }
            }
        }
    };
    tokio::select! {
        result = tcp_to_relay => result,
        result = relay_to_tcp => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tunnel_frames_are_strictly_bounded() {
        assert_eq!(NATIVE_TUNNEL_FRAME_BYTES, 16 * 1024);
    }
}
