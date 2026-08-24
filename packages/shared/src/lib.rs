use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

pub const MAX_RELAY_TICKET_LIFETIME_SECONDS: i64 = 300;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayTicket {
    pub room_id: String,
    pub user_id: String,
    pub expires_at: i64,
    pub signature: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RelayTicketError {
    #[error("relay ticket identifiers must not be empty")]
    EmptyIdentifier,
    #[error("relay ticket secret must contain at least 32 bytes")]
    WeakSecret,
    #[error("relay ticket has expired")]
    Expired,
    #[error("relay ticket lifetime exceeds the permitted maximum")]
    ExcessiveLifetime,
    #[error("relay ticket signature is invalid")]
    InvalidSignature,
}

impl RelayTicket {
    pub fn issue(
        secret: &[u8],
        room_id: &str,
        user_id: &str,
        expires_at: i64,
    ) -> Result<Self, RelayTicketError> {
        validate_inputs(secret, room_id, user_id)?;
        let signature = signature(secret, room_id, user_id, expires_at)?;
        Ok(Self {
            room_id: room_id.to_owned(),
            user_id: user_id.to_owned(),
            expires_at,
            signature: hex::encode(signature),
        })
    }

    pub fn verify(&self, secret: &[u8], now: i64) -> Result<(), RelayTicketError> {
        validate_inputs(secret, &self.room_id, &self.user_id)?;
        if self.expires_at < now {
            return Err(RelayTicketError::Expired);
        }
        if self.expires_at - now > MAX_RELAY_TICKET_LIFETIME_SECONDS {
            return Err(RelayTicketError::ExcessiveLifetime);
        }
        let provided =
            hex::decode(&self.signature).map_err(|_| RelayTicketError::InvalidSignature)?;
        let mut mac = relay_mac(secret)?;
        mac.update(&canonical_claims(
            &self.room_id,
            &self.user_id,
            self.expires_at,
        ));
        mac.verify_slice(&provided)
            .map_err(|_| RelayTicketError::InvalidSignature)
    }
}

fn validate_inputs(secret: &[u8], room_id: &str, user_id: &str) -> Result<(), RelayTicketError> {
    if secret.len() < 32 {
        return Err(RelayTicketError::WeakSecret);
    }
    if room_id.trim().is_empty() || user_id.trim().is_empty() {
        return Err(RelayTicketError::EmptyIdentifier);
    }
    Ok(())
}

fn relay_mac(secret: &[u8]) -> Result<Hmac<Sha256>, RelayTicketError> {
    Hmac::<Sha256>::new_from_slice(secret).map_err(|_| RelayTicketError::WeakSecret)
}

fn signature(
    secret: &[u8],
    room_id: &str,
    user_id: &str,
    expires_at: i64,
) -> Result<Vec<u8>, RelayTicketError> {
    let mut mac = relay_mac(secret)?;
    mac.update(&canonical_claims(room_id, user_id, expires_at));
    Ok(mac.finalize().into_bytes().to_vec())
}

fn canonical_claims(room_id: &str, user_id: &str, expires_at: i64) -> Vec<u8> {
    format!(
        "{}:{room_id}:{}:{user_id}:{expires_at}",
        room_id.len(),
        user_id.len()
    )
    .into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &[u8] = b"relay-test-secret-at-least-32-bytes-long";

    #[test]
    fn issues_and_verifies_a_short_lived_ticket() {
        let ticket = RelayTicket::issue(SECRET, "room", "user", 1_120).expect("ticket");
        assert_eq!(ticket.verify(SECRET, 1_000), Ok(()));
    }

    #[test]
    fn rejects_tampering_expiry_and_excessive_lifetime() {
        let mut ticket = RelayTicket::issue(SECRET, "room", "user", 1_120).expect("ticket");
        ticket.room_id = "other".into();
        assert_eq!(
            ticket.verify(SECRET, 1_000),
            Err(RelayTicketError::InvalidSignature)
        );

        let ticket = RelayTicket::issue(SECRET, "room", "user", 999).expect("ticket");
        assert_eq!(ticket.verify(SECRET, 1_000), Err(RelayTicketError::Expired));

        let ticket = RelayTicket::issue(SECRET, "room", "user", 2_000).expect("ticket");
        assert_eq!(
            ticket.verify(SECRET, 1_000),
            Err(RelayTicketError::ExcessiveLifetime)
        );
    }
}
