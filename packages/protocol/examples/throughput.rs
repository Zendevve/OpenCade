use std::{hint::black_box, time::Instant};

use opencade_protocol::{BorrowedEnvelope, Envelope, MatchEndpointPayload, NatMappingState};

const ITERATIONS: usize = 500_000;

fn main() {
    let envelope = Envelope::new(
        "match.endpoint",
        MatchEndpointPayload {
            room_id: "fb3a171c-82e7-4f27-a130-68cff7085737".into(),
            endpoint: "192.168.1.20:42000".into(),
            reflexive_endpoint: Some("203.0.113.9:52000".into()),
            nat: NatMappingState::Mapped,
            nonce: "8a1110d5-8dd2-4ad2-9c88-ad9768bc4905".into(),
        },
    );
    let encoded = serde_json::to_string(&envelope).expect("serialize fixture");

    let started = Instant::now();
    for _ in 0..ITERATIONS {
        let parsed: Envelope<serde_json::Value> =
            serde_json::from_str(black_box(&encoded)).expect("parse fixture");
        let payload: MatchEndpointPayload =
            serde_json::from_value(black_box(parsed.payload.clone())).expect("type payload");
        black_box(payload);
    }
    let elapsed = started.elapsed();
    let per_second = ITERATIONS as f64 / elapsed.as_secs_f64();
    println!(
        "legacy envelope parse + cloned typed payload: {per_second:.0} msg/s ({:.0} ns/msg)",
        elapsed.as_nanos() as f64 / ITERATIONS as f64
    );

    let started = Instant::now();
    for _ in 0..ITERATIONS {
        let parsed = BorrowedEnvelope::parse(black_box(&encoded)).expect("parse borrowed fixture");
        let payload: MatchEndpointPayload = parsed.payload_as().expect("type borrowed payload");
        black_box(payload);
    }
    let elapsed = started.elapsed();
    let per_second = ITERATIONS as f64 / elapsed.as_secs_f64();
    println!(
        "borrowed envelope + typed payload: {per_second:.0} msg/s ({:.0} ns/msg)",
        elapsed.as_nanos() as f64 / ITERATIONS as f64
    );
}
