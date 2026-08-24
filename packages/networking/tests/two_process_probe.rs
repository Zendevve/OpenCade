use opencade_networking::MatchProbeReport;
use std::net::{SocketAddr, UdpSocket};
use std::process::{Child, Command, Output, Stdio};

fn reserve_addresses() -> (SocketAddr, SocketAddr) {
    let host = UdpSocket::bind("127.0.0.1:0").expect("reserve host address");
    let guest = UdpSocket::bind("127.0.0.1:0").expect("reserve guest address");
    let host_address = host.local_addr().expect("read host address");
    let guest_address = guest.local_addr().expect("read guest address");
    (host_address, guest_address)
}

fn spawn_probe(
    local: SocketAddr,
    peer: SocketAddr,
    local_user: &str,
    peer_user: &str,
    role: &str,
) -> Child {
    Command::new(env!("CARGO_BIN_EXE_opencade-match-probe"))
        .args([
            "--local",
            &local.to_string(),
            "--peer",
            &peer.to_string(),
            "--room",
            "two-process-room",
            "--game",
            "sfiii3",
            "--local-user",
            local_user,
            "--peer-user",
            peer_user,
            "--role",
            role,
            "--session-key",
            "two-process-session",
            "--frames",
            "60",
            "--timeout-ms",
            "5000",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn match probe")
}

fn successful_report(output: Output) -> MatchProbeReport {
    assert!(
        output.status.success(),
        "probe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("valid probe report")
}

#[test]
fn independent_processes_complete_the_same_udp_transcript() {
    let (host_address, guest_address) = reserve_addresses();
    assert_ne!(host_address, guest_address);

    let host = spawn_probe(host_address, guest_address, "host", "guest", "host");
    let guest = spawn_probe(guest_address, host_address, "guest", "host", "guest");
    let host_report = successful_report(host.wait_with_output().expect("host output"));
    let guest_report = successful_report(guest.wait_with_output().expect("guest output"));

    assert_eq!(host_report.frames_received, 60);
    assert_eq!(guest_report.frames_received, 60);
    assert_eq!(
        host_report.transcript_checksum,
        guest_report.transcript_checksum
    );
    assert_eq!(host_report.room_id, "two-process-room");
    assert_eq!(guest_report.room_id, "two-process-room");
}
