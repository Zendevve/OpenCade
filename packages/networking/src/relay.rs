use super::{TransportError, probe::ProbePacket};
use futures_util::{SinkExt, StreamExt};
use opencade_shared::RelayTicket;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};
use url::Url;

pub struct RelayPeer {
    stream: Mutex<WebSocketStream<MaybeTlsStream<TcpStream>>>,
}

impl RelayPeer {
    pub async fn connect(relay_url: &str, ticket: &RelayTicket) -> Result<Self, TransportError> {
        let mut url = Url::parse(relay_url)
            .map_err(|_| TransportError::Relay("relay URL is invalid".into()))?;
        if !matches!(url.scheme(), "ws" | "wss") {
            return Err(TransportError::Relay("relay URL must use ws or wss".into()));
        }
        url.query_pairs_mut()
            .append_pair("room_id", &ticket.room_id)
            .append_pair("user_id", &ticket.user_id)
            .append_pair("expires_at", &ticket.expires_at.to_string())
            .append_pair("signature", &ticket.signature);
        let (stream, _) = connect_async(url.as_str())
            .await
            .map_err(|_| TransportError::Relay("relay connection failed".into()))?;
        Ok(Self {
            stream: Mutex::new(stream),
        })
    }

    pub(super) async fn send_packet(&self, packet: &ProbePacket) -> Result<(), TransportError> {
        let encoded = serde_json::to_vec(packet)
            .map_err(|error| TransportError::Serialization(error.to_string()))?;
        self.stream
            .lock()
            .await
            .send(Message::Binary(encoded.into()))
            .await
            .map_err(|_| TransportError::Relay("relay send failed".into()))
    }

    pub(super) async fn receive_packet(&self) -> Result<ProbePacket, TransportError> {
        let mut stream = self.stream.lock().await;
        loop {
            match stream.next().await {
                Some(Ok(Message::Binary(payload))) => {
                    return serde_json::from_slice(&payload)
                        .map_err(|error| TransportError::Serialization(error.to_string()));
                }
                Some(Ok(Message::Ping(payload))) => stream
                    .send(Message::Pong(payload))
                    .await
                    .map_err(|_| TransportError::Relay("relay pong failed".into()))?,
                Some(Ok(Message::Pong(_))) => {}
                Some(Ok(Message::Text(_))) => {
                    return Err(TransportError::Relay(
                        "relay returned an unexpected text frame".into(),
                    ));
                }
                Some(Ok(Message::Close(_))) | None => {
                    return Err(TransportError::Relay("relay connection closed".into()));
                }
                Some(Ok(Message::Frame(_))) => {}
                Some(Err(_)) => {
                    return Err(TransportError::Relay("relay receive failed".into()));
                }
            }
        }
    }
}
