use crate::{TransportError, UdpPeer};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;
use uuid::Uuid;

const BINDING_REQUEST: u16 = 0x0001;
const BINDING_SUCCESS: u16 = 0x0101;
const MAGIC_COOKIE: u32 = 0x2112_A442;
const MAPPED_ADDRESS: u16 = 0x0001;
const XOR_MAPPED_ADDRESS: u16 = 0x0020;
const HEADER_BYTES: usize = 20;
const MAX_STUN_BYTES: usize = 2_048;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NatMapping {
    Open,
    Mapped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StunObservation {
    pub local_endpoint: SocketAddr,
    pub reflexive_endpoint: SocketAddr,
    pub mapping: NatMapping,
}

/// Uses the already-reserved match socket for an RFC 8489 Binding transaction. Reusing the socket
/// is essential: a reflexive address learned from a throwaway socket is not the match mapping.
pub async fn discover_reflexive_address(
    peer: &UdpPeer,
    server: SocketAddr,
    timeout: Duration,
) -> Result<StunObservation, TransportError> {
    if server.ip().is_unspecified() || server.port() == 0 || timeout.is_zero() {
        return Err(TransportError::InvalidConfiguration(
            "STUN server and timeout must be usable".into(),
        ));
    }
    let local_endpoint = peer.local_addr()?;
    let transaction_id = transaction_id();
    let request = binding_request(transaction_id);
    peer.socket
        .send_to(&request, server)
        .await
        .map_err(super::map_udp_error)?;

    let deadline = tokio::time::Instant::now() + timeout;
    let mut buffer = [0_u8; MAX_STUN_BYTES];
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(TransportError::StunTimeout);
        }
        let received = tokio::time::timeout(remaining, peer.socket.recv_from(&mut buffer))
            .await
            .map_err(|_| TransportError::StunTimeout)?
            .map_err(super::map_udp_error)?;
        if received.1 != server {
            continue;
        }
        let reflexive_endpoint = parse_binding_response(&buffer[..received.0], transaction_id)?;
        let mapping = if reflexive_endpoint == local_endpoint {
            NatMapping::Open
        } else {
            NatMapping::Mapped
        };
        return Ok(StunObservation {
            local_endpoint,
            reflexive_endpoint,
            mapping,
        });
    }
}

fn transaction_id() -> [u8; 12] {
    let uuid = Uuid::new_v4();
    let mut transaction_id = [0_u8; 12];
    transaction_id.copy_from_slice(&uuid.as_bytes()[..12]);
    transaction_id
}

fn binding_request(transaction_id: [u8; 12]) -> [u8; HEADER_BYTES] {
    let mut request = [0_u8; HEADER_BYTES];
    request[..2].copy_from_slice(&BINDING_REQUEST.to_be_bytes());
    request[4..8].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());
    request[8..].copy_from_slice(&transaction_id);
    request
}

fn parse_binding_response(
    response: &[u8],
    transaction_id: [u8; 12],
) -> Result<SocketAddr, TransportError> {
    if response.len() < HEADER_BYTES {
        return invalid("response is shorter than the STUN header");
    }
    if u16::from_be_bytes([response[0], response[1]]) != BINDING_SUCCESS {
        return invalid("response is not a Binding success response");
    }
    if u32::from_be_bytes([response[4], response[5], response[6], response[7]]) != MAGIC_COOKIE {
        return invalid("magic cookie does not match RFC 8489");
    }
    if response[8..20] != transaction_id {
        return invalid("transaction id does not match the request");
    }
    let message_length = usize::from(u16::from_be_bytes([response[2], response[3]]));
    if message_length % 4 != 0 {
        return invalid("declared message length is not 32-bit aligned");
    }
    let message_end = HEADER_BYTES
        .checked_add(message_length)
        .filter(|end| *end <= response.len())
        .ok_or_else(|| {
            TransportError::InvalidStunResponse("declared message length is invalid".into())
        })?;

    let mut offset = HEADER_BYTES;
    let mut mapped = None;
    while offset + 4 <= message_end {
        let attribute_type = u16::from_be_bytes([response[offset], response[offset + 1]]);
        let attribute_length = usize::from(u16::from_be_bytes([
            response[offset + 2],
            response[offset + 3],
        ]));
        let value_start = offset + 4;
        let value_end = value_start
            .checked_add(attribute_length)
            .filter(|end| *end <= message_end)
            .ok_or_else(|| {
                TransportError::InvalidStunResponse("attribute length is invalid".into())
            })?;
        if attribute_type == XOR_MAPPED_ADDRESS {
            return parse_address(&response[value_start..value_end], Some(transaction_id));
        }
        if attribute_type == MAPPED_ADDRESS {
            mapped = Some(parse_address(&response[value_start..value_end], None)?);
        }
        offset = value_end
            .checked_add((4 - attribute_length % 4) % 4)
            .ok_or_else(|| {
                TransportError::InvalidStunResponse("attribute padding overflowed".into())
            })?;
    }
    mapped.ok_or_else(|| {
        TransportError::InvalidStunResponse("response contains no mapped address".into())
    })
}

fn parse_address(
    value: &[u8],
    xor_transaction_id: Option<[u8; 12]>,
) -> Result<SocketAddr, TransportError> {
    if value.len() < 8 || value[0] != 0 {
        return invalid("mapped address attribute is malformed");
    }
    let encoded_port = u16::from_be_bytes([value[2], value[3]]);
    let port = xor_transaction_id
        .map(|_| encoded_port ^ (MAGIC_COOKIE >> 16) as u16)
        .unwrap_or(encoded_port);
    let cookie = MAGIC_COOKIE.to_be_bytes();
    let ip = match value[1] {
        0x01 if value.len() == 8 => {
            let mut octets = [0_u8; 4];
            octets.copy_from_slice(&value[4..8]);
            if xor_transaction_id.is_some() {
                for (octet, mask) in octets.iter_mut().zip(cookie) {
                    *octet ^= mask;
                }
            }
            IpAddr::V4(Ipv4Addr::from(octets))
        }
        0x02 if value.len() == 20 => {
            let mut octets = [0_u8; 16];
            octets.copy_from_slice(&value[4..20]);
            if let Some(transaction_id) = xor_transaction_id {
                for (octet, mask) in octets
                    .iter_mut()
                    .zip(cookie.into_iter().chain(transaction_id))
                {
                    *octet ^= mask;
                }
            }
            IpAddr::V6(Ipv6Addr::from(octets))
        }
        _ => return invalid("mapped address family or length is unsupported"),
    };
    Ok(SocketAddr::new(ip, port))
}

fn invalid<T>(message: &str) -> Result<T, TransportError> {
    Err(TransportError::InvalidStunResponse(message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xor_mapped_response(transaction_id: [u8; 12], address: SocketAddr) -> Vec<u8> {
        let mut response = Vec::from(binding_request(transaction_id));
        response[..2].copy_from_slice(&BINDING_SUCCESS.to_be_bytes());
        response[2..4].copy_from_slice(&12_u16.to_be_bytes());
        response.extend_from_slice(&XOR_MAPPED_ADDRESS.to_be_bytes());
        response.extend_from_slice(&8_u16.to_be_bytes());
        response.extend_from_slice(&[0, 1]);
        response.extend_from_slice(&(address.port() ^ (MAGIC_COOKIE >> 16) as u16).to_be_bytes());
        let IpAddr::V4(ip) = address.ip() else {
            panic!("IPv4 fixture");
        };
        for (octet, mask) in ip.octets().into_iter().zip(MAGIC_COOKIE.to_be_bytes()) {
            response.push(octet ^ mask);
        }
        response
    }

    #[test]
    fn parses_xor_mapped_ipv4_response() {
        let transaction_id = [7_u8; 12];
        let address: SocketAddr = "203.0.113.9:45678".parse().expect("address");
        let response = xor_mapped_response(transaction_id, address);

        assert_eq!(
            parse_binding_response(&response, transaction_id).expect("binding response"),
            address
        );
    }

    #[tokio::test]
    async fn binding_transaction_uses_the_reserved_match_socket() {
        let server = tokio::net::UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("mock STUN server");
        let server_address = server.local_addr().expect("server address");
        let responder = tokio::spawn(async move {
            let mut request = [0_u8; HEADER_BYTES];
            let (_, source) = server
                .recv_from(&mut request)
                .await
                .expect("Binding request");
            let transaction_id: [u8; 12] = request[8..20].try_into().expect("transaction id");
            let response = xor_mapped_response(transaction_id, source);
            server
                .send_to(&response, source)
                .await
                .expect("Binding response");
            source
        });
        let peer = UdpPeer::bind_unconnected("127.0.0.1:0".parse().expect("peer address"))
            .await
            .expect("reserved peer");
        let reserved_address = peer.local_addr().expect("reserved address");
        let observation =
            discover_reflexive_address(&peer, server_address, Duration::from_millis(500))
                .await
                .expect("STUN observation");

        assert_eq!(responder.await.expect("responder"), reserved_address);
        assert_eq!(observation.reflexive_endpoint, reserved_address);
        assert_eq!(observation.mapping, NatMapping::Open);
    }

    #[test]
    fn rejects_wrong_transaction_and_truncated_attributes() {
        let transaction_id = [7_u8; 12];
        let response = binding_request(transaction_id);
        assert!(parse_binding_response(&response, [8_u8; 12]).is_err());

        let mut truncated = Vec::from(response);
        truncated[..2].copy_from_slice(&BINDING_SUCCESS.to_be_bytes());
        truncated[2..4].copy_from_slice(&8_u16.to_be_bytes());
        assert!(parse_binding_response(&truncated, transaction_id).is_err());
    }
}
