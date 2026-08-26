use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
};
use futures_util::{SinkExt, StreamExt};
use opencade_protocol::Envelope;
use opencade_server::{AppState, Config, build_app};
use opencade_shared::RelayTicket;
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tower::ServiceExt;

async fn request(
    app: &Router,
    method: Method,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Envelope<Value>) {
    request_with_operator_token(app, method, uri, token, body, None).await
}

async fn operator_request(
    app: &Router,
    method: Method,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Envelope<Value>) {
    request_with_operator_token(
        app,
        method,
        uri,
        token,
        body,
        Some("test-operator-token-with-32-characters"),
    )
    .await
}

async fn request_with_operator_token(
    app: &Router,
    method: Method,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
    operator_token: Option<&str>,
) -> (StatusCode, Envelope<Value>) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(operator_token) = operator_token {
        builder = builder.header("x-operator-token", operator_token);
    }
    let request_body = match body {
        Some(value) => {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            Body::from(serde_json::to_vec(&value).expect("serialize request"))
        }
        None => Body::empty(),
    };
    let response = app
        .clone()
        .oneshot(builder.body(request_body).expect("build request"))
        .await
        .expect("router response");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read response body");
    let envelope = serde_json::from_slice(&bytes).expect("parse response envelope");
    (status, envelope)
}

async fn register(app: &Router, username: &str) -> String {
    let (status, response) = request(
        app,
        Method::POST,
        "/api/v1/auth/register",
        None,
        Some(json!({
            "username": username,
            "email": format!("{username}@example.com"),
            "password": "correct-horse-battery-staple"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    response.payload["token"]
        .as_str()
        .expect("registration token")
        .to_string()
}

async fn current_user_id(app: &Router, token: &str) -> String {
    let (status, response) = request(app, Method::GET, "/api/v1/auth/me", Some(token), None).await;
    assert_eq!(status, StatusCode::OK);
    response.payload["user"]["id"]
        .as_str()
        .expect("current user id")
        .to_string()
}

fn telemetry_event(
    event_id: uuid::Uuid,
    session_id: uuid::Uuid,
    event: &str,
    blocked_checks: &[&str],
) -> Value {
    json!({
        "event_id": event_id,
        "anonymous_session_id": session_id,
        "event": event,
        "game_id": "sfiii3",
        "blocked_checks": blocked_checks
    })
}

#[sqlx::test]
async fn authenticated_users_create_and_accept_a_room(pool: PgPool) {
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations should succeed");
    let mut config = Config::for_test();
    config.relay_url = Some("wss://relay.example/relay".into());
    config.relay_secret = Some("integration-relay-secret-at-least-32-bytes".into());
    let app = build_app(AppState::new(pool.clone(), config));
    let host_token = register(&app, "host_player").await;
    let guest_token = register(&app, "guest_player").await;
    let observer_token = register(&app, "observer_player").await;
    let guest_id = current_user_id(&app, &guest_token).await;

    let (games_status, games) =
        request(&app, Method::GET, "/api/v1/games", Some(&host_token), None).await;
    assert_eq!(games_status, StatusCode::OK);
    assert_eq!(games.payload["games"].as_array().map(Vec::len), Some(5));

    let (oversized_room_status, _) = request(
        &app,
        Method::POST,
        "/api/v1/rooms",
        Some(&host_token),
        Some(json!({ "game_id": "sfiii3", "max_players": 3 })),
    )
    .await;
    assert_eq!(oversized_room_status, StatusCode::BAD_REQUEST);

    let (_, waiting_room) = request(
        &app,
        Method::POST,
        "/api/v1/rooms",
        Some(&host_token),
        Some(json!({ "game_id": "sfiii3" })),
    )
    .await;
    let (existing_status, existing_room) = request(
        &app,
        Method::POST,
        "/api/v1/rooms",
        Some(&host_token),
        Some(json!({ "game_id": "sfiii3" })),
    )
    .await;
    assert_eq!(existing_status, StatusCode::OK);
    assert_eq!(existing_room.payload["id"], waiting_room.payload["id"]);

    let (create_status, created) = request(
        &app,
        Method::POST,
        "/api/v1/challenges",
        Some(&host_token),
        Some(json!({ "game_id": "sfiii3", "challenged_id": guest_id })),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED);
    assert_eq!(created.payload["state"], "pending");
    let challenge_id = created.payload["id"].as_str().expect("challenge id");

    let (incoming_status, incoming) = request(
        &app,
        Method::GET,
        "/api/v1/challenges",
        Some(&guest_token),
        None,
    )
    .await;
    assert_eq!(incoming_status, StatusCode::OK);
    assert_eq!(
        incoming.payload["challenges"].as_array().map(Vec::len),
        Some(1)
    );

    let (unauthorized_status, unauthorized) = request(
        &app,
        Method::POST,
        &format!("/api/v1/challenges/{challenge_id}/accept"),
        Some(&observer_token),
        None,
    )
    .await;
    assert_eq!(unauthorized_status, StatusCode::FORBIDDEN);
    assert_eq!(unauthorized.payload["code"], "forbidden");

    let (accept_status, accepted) = request(
        &app,
        Method::POST,
        &format!("/api/v1/challenges/{challenge_id}/accept"),
        Some(&guest_token),
        None,
    )
    .await;
    assert_eq!(accept_status, StatusCode::OK);
    assert_eq!(accepted.payload["state"], "accepted");
    let room_id = accepted.payload["room_id"].as_str().expect("room id");
    let (repeat_accept_status, repeat_accept) = request(
        &app,
        Method::POST,
        &format!("/api/v1/challenges/{challenge_id}/accept"),
        Some(&guest_token),
        None,
    )
    .await;
    assert_eq!(repeat_accept_status, StatusCode::OK);
    assert_eq!(repeat_accept.payload["room_id"], room_id);

    let (room_status, room) = request(
        &app,
        Method::GET,
        &format!("/api/v1/rooms/{room_id}"),
        Some(&host_token),
        None,
    )
    .await;
    assert_eq!(room_status, StatusCode::OK);
    assert_eq!(room.payload["state"], "connecting");
    assert!(room.payload["guest_id"].as_str().is_some());

    let (observer_relay_status, _) = request(
        &app,
        Method::POST,
        &format!("/api/v1/rooms/{room_id}/relay-ticket"),
        Some(&observer_token),
        None,
    )
    .await;
    assert_eq!(observer_relay_status, StatusCode::FORBIDDEN);
    let (relay_status, relay_response) = request(
        &app,
        Method::POST,
        &format!("/api/v1/rooms/{room_id}/relay-ticket"),
        Some(&host_token),
        None,
    )
    .await;
    assert_eq!(relay_status, StatusCode::OK);
    assert_eq!(
        relay_response.payload["relay_url"],
        "wss://relay.example/relay"
    );
    let relay_ticket: RelayTicket =
        serde_json::from_value(relay_response.payload["ticket"].clone()).expect("relay ticket");
    assert_eq!(relay_ticket.room_id, room_id);
    assert!(
        relay_ticket
            .verify(
                b"integration-relay-secret-at-least-32-bytes",
                chrono::Utc::now().timestamp(),
            )
            .is_ok()
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test server");
    let address = listener.local_addr().expect("test server address");
    let server_app = app.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, server_app)
            .await
            .expect("test server should run");
    });
    let mut host_request = format!("ws://{address}/ws")
        .into_client_request()
        .expect("host websocket request");
    host_request.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        format!("opencade.v1, opencade.auth.{host_token}")
            .parse()
            .expect("host protocol header"),
    );
    let mut guest_request = format!("ws://{address}/ws")
        .into_client_request()
        .expect("guest websocket request");
    guest_request.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        format!("opencade.v1, opencade.auth.{guest_token}")
            .parse()
            .expect("guest protocol header"),
    );
    let (mut host_socket, _) = connect_async(host_request).await.expect("host websocket");
    let (mut guest_socket, _) = connect_async(guest_request).await.expect("guest websocket");
    let _host_hello = host_socket
        .next()
        .await
        .expect("host hello")
        .expect("host frame");
    let _guest_hello = guest_socket
        .next()
        .await
        .expect("guest hello")
        .expect("guest frame");

    let request_id = "signaling-integration-1";
    let offer = json!({
        "type": "signaling.offer",
        "version": "1.0",
        "request_id": request_id,
        "timestamp": chrono::Utc::now(),
        "payload": { "room_id": room_id, "sdp": "v=0\\r\\n" }
    });
    host_socket
        .send(Message::Text(offer.to_string().into()))
        .await
        .expect("send offer");
    let relayed = guest_socket
        .next()
        .await
        .expect("relayed offer")
        .expect("guest frame");
    let relayed: Envelope<Value> =
        serde_json::from_str(relayed.to_text().expect("text offer")).expect("relayed envelope");
    assert_eq!(relayed.msg_type, "signaling.offer");
    assert_eq!(relayed.request_id, request_id);
    let acknowledgement = host_socket
        .next()
        .await
        .expect("offer ack")
        .expect("host frame");
    let acknowledgement: Envelope<Value> =
        serde_json::from_str(acknowledgement.to_text().expect("text ack")).expect("ack envelope");
    assert_eq!(acknowledgement.msg_type, "signaling.relayed");
    assert_eq!(acknowledgement.request_id, request_id);

    let endpoint_request_id = "endpoint-integration-1";
    let endpoint = json!({
        "type": "match.endpoint",
        "version": "1.0",
        "request_id": endpoint_request_id,
        "timestamp": chrono::Utc::now(),
        "payload": {
            "room_id": room_id,
            "endpoint": "192.168.1.20:42000",
            "nonce": "8a1110d5-8dd2-4ad2-9c88-ad9768bc4905"
        }
    });
    host_socket
        .send(Message::Text(endpoint.to_string().into()))
        .await
        .expect("send endpoint");
    let relayed_endpoint = guest_socket
        .next()
        .await
        .expect("relayed endpoint")
        .expect("guest endpoint frame");
    let relayed_endpoint: Envelope<Value> =
        serde_json::from_str(relayed_endpoint.to_text().expect("text endpoint"))
            .expect("endpoint envelope");
    assert_eq!(relayed_endpoint.msg_type, "match.endpoint");
    assert_eq!(relayed_endpoint.request_id, endpoint_request_id);
    assert_eq!(relayed_endpoint.payload["endpoint"], "192.168.1.20:42000");
    let endpoint_ack = host_socket
        .next()
        .await
        .expect("endpoint ack")
        .expect("host endpoint frame");
    let endpoint_ack: Envelope<Value> =
        serde_json::from_str(endpoint_ack.to_text().expect("text endpoint ack"))
            .expect("endpoint ack envelope");
    assert_eq!(endpoint_ack.msg_type, "match.endpoint.relayed");
    assert_eq!(endpoint_ack.request_id, endpoint_request_id);

    let completion_request_id = "completion-integration-1";
    let completion = json!({
        "type": "match.probe.completed",
        "version": "1.0",
        "request_id": completion_request_id,
        "timestamp": chrono::Utc::now(),
        "payload": {
            "room_id": room_id,
            "frames_received": 60,
            "transcript_checksum": "0376c2e852f4fd25"
        }
    });
    guest_socket
        .send(Message::Text(completion.to_string().into()))
        .await
        .expect("send completion");
    let relayed_completion = host_socket
        .next()
        .await
        .expect("relayed completion")
        .expect("host completion frame");
    let relayed_completion: Envelope<Value> =
        serde_json::from_str(relayed_completion.to_text().expect("text completion"))
            .expect("completion envelope");
    assert_eq!(relayed_completion.msg_type, "match.probe.completed");
    assert_eq!(relayed_completion.request_id, completion_request_id);
    let completion_ack = guest_socket
        .next()
        .await
        .expect("completion ack")
        .expect("guest completion frame");
    let completion_ack: Envelope<Value> =
        serde_json::from_str(completion_ack.to_text().expect("text completion ack"))
            .expect("completion ack envelope");
    assert_eq!(completion_ack.msg_type, "match.probe.completed.relayed");
    assert_eq!(completion_ack.request_id, completion_request_id);

    let (grant_status, grant) = request(
        &app,
        Method::POST,
        &format!("/api/v1/rooms/{room_id}/launch-grant"),
        Some(&host_token),
        Some(json!({
            "local_endpoint": "192.168.1.20:55435",
            "peer_endpoint": "192.168.1.21:55435",
            "input_delay_frames": 2
        })),
    )
    .await;
    assert_eq!(grant_status, StatusCode::OK);
    let raw_grant = grant.payload["grant"].as_str().expect("launch grant");
    let (consume_status, consumed) = request(
        &app,
        Method::POST,
        "/api/v1/match-launch-grants/consume",
        Some(&host_token),
        Some(json!({ "grant": raw_grant })),
    )
    .await;
    assert_eq!(consume_status, StatusCode::OK);
    assert_eq!(consumed.payload["room_id"], room_id);
    assert_eq!(consumed.payload["role"], "host");
    let (replay_status, _) = request(
        &app,
        Method::POST,
        "/api/v1/match-launch-grants/consume",
        Some(&host_token),
        Some(json!({ "grant": raw_grant })),
    )
    .await;
    assert_eq!(replay_status, StatusCode::UNAUTHORIZED);
    let (guest_grant_status, guest_grant) = request(
        &app,
        Method::POST,
        &format!("/api/v1/rooms/{room_id}/launch-grant"),
        Some(&guest_token),
        Some(json!({
            "local_endpoint": "192.168.1.21:55435",
            "peer_endpoint": "192.168.1.20:55435",
            "input_delay_frames": 2
        })),
    )
    .await;
    assert_eq!(guest_grant_status, StatusCode::OK);
    let (guest_consume_status, guest_consumed) = request(
        &app,
        Method::POST,
        "/api/v1/match-launch-grants/consume",
        Some(&guest_token),
        Some(json!({ "grant": guest_grant.payload["grant"] })),
    )
    .await;
    assert_eq!(guest_consume_status, StatusCode::OK);
    assert_eq!(guest_consumed.payload["role"], "guest");

    let (start_status, started) = request(
        &app,
        Method::POST,
        &format!("/api/v1/rooms/{room_id}/start"),
        Some(&host_token),
        None,
    )
    .await;
    assert_eq!(start_status, StatusCode::OK);
    assert_eq!(started.payload["state"], "connecting");
    let (guest_start_status, started) = request(
        &app,
        Method::POST,
        &format!("/api/v1/rooms/{room_id}/start"),
        Some(&guest_token),
        None,
    )
    .await;
    assert_eq!(guest_start_status, StatusCode::OK);
    assert_eq!(started.payload["state"], "playing");
    let (finish_status, finished) = request(
        &app,
        Method::POST,
        &format!("/api/v1/rooms/{room_id}/finish"),
        Some(&host_token),
        Some(json!({ "exit_code": 0 })),
    )
    .await;
    assert_eq!(finish_status, StatusCode::OK);
    assert_eq!(finished.payload["state"], "playing");
    let (guest_finish_status, finished) = request(
        &app,
        Method::POST,
        &format!("/api/v1/rooms/{room_id}/finish"),
        Some(&guest_token),
        Some(json!({ "exit_code": 0 })),
    )
    .await;
    assert_eq!(guest_finish_status, StatusCode::OK);
    assert_eq!(finished.payload["state"], "finished");
    let ended = sqlx::query_scalar::<_, bool>(
        "SELECT ended_at IS NOT NULL FROM matches WHERE room_id = $1",
    )
    .bind(uuid::Uuid::parse_str(room_id).expect("room uuid"))
    .fetch_one(&pool)
    .await
    .expect("completed match row");
    assert!(ended);
    server.abort();
}

#[sqlx::test]
async fn expired_attempts_are_reconciled_authoritatively(pool: PgPool) {
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations should succeed");
    let state = AppState::new(pool.clone(), Config::for_test());
    let app = build_app(state.clone());
    let token = register(&app, "deadline_host").await;
    let (status, room) = request(
        &app,
        Method::POST,
        "/api/v1/rooms",
        Some(&token),
        Some(json!({ "game_id": "sfiii3" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let room_id = room.payload["id"].as_str().expect("room id");
    sqlx::query(
        "UPDATE rooms
         SET state = 'CONNECTING', state_deadline_at = now() - interval '1 second'
         WHERE id = $1",
    )
    .bind(uuid::Uuid::parse_str(room_id).expect("room uuid"))
    .execute(&pool)
    .await
    .expect("expire room");

    assert_eq!(
        opencade_server::lifecycle::reconcile_expired(&state)
            .await
            .expect("reconcile"),
        1
    );
    let outcome = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT rooms.state, match_attempts.state, match_attempts.failure_code
         FROM rooms JOIN match_attempts USING (attempt_id) WHERE rooms.id = $1",
    )
    .bind(uuid::Uuid::parse_str(room_id).expect("room uuid"))
    .fetch_one(&pool)
    .await
    .expect("attempt outcome");
    assert_eq!(outcome.0, "CANCELLED");
    assert_eq!(outcome.1, "EXPIRED");
    assert_eq!(outcome.2.as_deref(), Some("state_deadline_exceeded"));
}

#[sqlx::test]
async fn community_alpha_invite_preflight_barrier_and_evidence_flow(pool: PgPool) {
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations should succeed");
    let mut config = Config::for_test();
    config.relay_url = Some("wss://relay.example/relay".into());
    config.relay_secret = Some("integration-relay-secret-at-least-32-bytes".into());
    let app = build_app(AppState::new(pool, config));
    let host_token = register(&app, "campaign_host").await;
    let guest_token = register(&app, "campaign_guest").await;
    let observer_token = register(&app, "campaign_observer").await;

    let (_, room) = request(
        &app,
        Method::POST,
        "/api/v1/rooms",
        Some(&host_token),
        Some(json!({ "game_id": "sfiii3" })),
    )
    .await;
    let room_id = room.payload["id"].as_str().expect("room id");
    let (invite_status, invite) = request(
        &app,
        Method::POST,
        &format!("/api/v1/rooms/{room_id}/invite"),
        Some(&host_token),
        None,
    )
    .await;
    assert_eq!(invite_status, StatusCode::CREATED);
    let code = invite.payload["code"].as_str().expect("invite code");
    assert_eq!(code.len(), 10);
    let (join_status, joined) = request(
        &app,
        Method::POST,
        "/api/v1/invites/join",
        Some(&guest_token),
        Some(json!({ "code": code.to_ascii_lowercase() })),
    )
    .await;
    assert_eq!(join_status, StatusCode::OK);
    assert_eq!(joined.payload["state"], "connecting");
    let (replay_status, _) = request(
        &app,
        Method::POST,
        "/api/v1/invites/join",
        Some(&observer_token),
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(replay_status, StatusCode::NOT_FOUND);

    let compatibility = json!({
        "adapter": "retroarch_fbneo",
        "emulator_version": "1.22.0",
        "executable_sha256": "a".repeat(64),
        "core_sha256": "b".repeat(64),
        "content_sha256": "c".repeat(64)
    });
    for token in [&host_token, &guest_token] {
        let (status, _) = request(
            &app,
            Method::POST,
            &format!("/api/v1/rooms/{room_id}/preflight"),
            Some(token),
            Some(json!({
                "room_id": room_id,
                "compatibility": compatibility,
                "native_port_available": true
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }
    for token in [&host_token, &guest_token] {
        let (status, _) = request(
            &app,
            Method::POST,
            &format!("/api/v1/rooms/{room_id}/ready"),
            Some(token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }
    let (snapshot_status, snapshot) = request(
        &app,
        Method::GET,
        &format!("/api/v1/rooms/{room_id}/snapshot"),
        Some(&host_token),
        None,
    )
    .await;
    assert_eq!(snapshot_status, StatusCode::OK);
    assert_eq!(snapshot.payload["preflight_count"], 2);
    assert_eq!(snapshot.payload["compatibility_matched"], true);
    assert_eq!(snapshot.payload["barrier"]["ready_count"], 2);
    assert!(snapshot.payload["barrier"]["launch_at"].is_string());
    let launch_at = snapshot.payload["barrier"]["launch_at"].clone();
    let (duplicate_ready_status, duplicate_ready) = request(
        &app,
        Method::POST,
        &format!("/api/v1/rooms/{room_id}/ready"),
        Some(&host_token),
        None,
    )
    .await;
    assert_eq!(duplicate_ready_status, StatusCode::OK);
    assert_eq!(duplicate_ready.payload["barrier"]["launch_at"], launch_at);

    let (tunnel_status, tunnel) = request(
        &app,
        Method::POST,
        &format!("/api/v1/rooms/{room_id}/native-tunnel-ticket"),
        Some(&host_token),
        None,
    )
    .await;
    assert_eq!(tunnel_status, StatusCode::OK);
    assert_eq!(tunnel.payload["ticket"]["capability"], "native_tcp_tunnel");

    let failure = json!({
        "schema_version": 1,
        "kind": "attempt_failure",
        "exported_at": chrono::Utc::now(),
        "room": { "id": room_id, "game_id": "sfiii3", "state": "connecting" },
        "role": "host",
        "stage": "direct_udp",
        "error_code": "direct_udp_timeout",
        "transport": "direct_udp",
        "client": { "platform": "macos", "user_agent": "integration-test" }
    });
    let (evidence_status, evidence) = request(
        &app,
        Method::POST,
        "/api/v1/alpha/evidence",
        Some(&host_token),
        Some(json!({ "evidence": failure.clone() })),
    )
    .await;
    assert_eq!(evidence_status, StatusCode::CREATED);
    assert_eq!(evidence.payload["duplicate"], false);
    let (duplicate_status, duplicate) = request(
        &app,
        Method::POST,
        "/api/v1/alpha/evidence",
        Some(&host_token),
        Some(json!({ "evidence": failure.clone() })),
    )
    .await;
    assert_eq!(duplicate_status, StatusCode::OK);
    assert_eq!(duplicate.payload["duplicate"], true);
    let mut conflicting_failure = failure;
    conflicting_failure["error_code"] = json!("different_failure");
    let (conflict_status, _) = request(
        &app,
        Method::POST,
        "/api/v1/alpha/evidence",
        Some(&host_token),
        Some(json!({ "evidence": conflicting_failure })),
    )
    .await;
    assert_eq!(conflict_status, StatusCode::CONFLICT);

    let (privacy_status, _) = request(
        &app,
        Method::POST,
        "/api/v1/alpha/evidence",
        Some(&host_token),
        Some(json!({
            "evidence": {
                "kind": "attempt_failure",
                "endpoint": "192.168.1.20:55435"
            }
        })),
    )
    .await;
    assert_eq!(privacy_status, StatusCode::BAD_REQUEST);
    let (campaign_status, campaign) = operator_request(
        &app,
        Method::GET,
        "/api/v1/alpha/campaign",
        Some(&host_token),
        None,
    )
    .await;
    assert_eq!(campaign_status, StatusCode::OK);
    assert_eq!(campaign.payload["attempts"], 1);
    assert_eq!(campaign.payload["failed"], 1);
}

#[sqlx::test]
async fn logout_revokes_the_current_session(pool: PgPool) {
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations should succeed");
    let app = build_app(AppState::new(pool, Config::for_test()));
    let token = register(&app, "logout_player").await;

    let (logout_status, _) = request(
        &app,
        Method::POST,
        "/api/v1/auth/logout",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(logout_status, StatusCode::OK);

    let (games_status, response) =
        request(&app, Method::GET, "/api/v1/games", Some(&token), None).await;
    assert_eq!(games_status, StatusCode::UNAUTHORIZED);
    assert_eq!(response.payload["code"], "unauthorized");
}

#[sqlx::test]
async fn product_telemetry_is_private_idempotent_bounded_and_aggregated(pool: PgPool) {
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations should succeed");
    let app = build_app(AppState::new(pool.clone(), Config::for_test()));
    let token = register(&app, "telemetry_player").await;
    let session_id = uuid::Uuid::new_v4();
    let selected_id = uuid::Uuid::new_v4();

    let (unauthorized, _) = request(
        &app,
        Method::POST,
        "/api/v1/telemetry/events",
        None,
        Some(telemetry_event(
            selected_id,
            session_id,
            "game_selected",
            &[],
        )),
    )
    .await;
    assert_eq!(unauthorized, StatusCode::UNAUTHORIZED);

    for (event_id, event) in [
        (selected_id, "game_selected"),
        (uuid::Uuid::new_v4(), "readiness_completed"),
        (uuid::Uuid::new_v4(), "lobby_entered"),
    ] {
        let (status, accepted) = request(
            &app,
            Method::POST,
            "/api/v1/telemetry/events",
            Some(&token),
            Some(telemetry_event(event_id, session_id, event, &[])),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(accepted.payload["duplicate"], false);
    }
    for _ in 0..2 {
        let additional_session = uuid::Uuid::new_v4();
        for event in ["game_selected", "readiness_completed", "lobby_entered"] {
            let (status, _) = request(
                &app,
                Method::POST,
                "/api/v1/telemetry/events",
                Some(&token),
                Some(telemetry_event(
                    uuid::Uuid::new_v4(),
                    additional_session,
                    event,
                    &[],
                )),
            )
            .await;
            assert_eq!(status, StatusCode::CREATED);
        }
    }
    let (duplicate_status, duplicate) = request(
        &app,
        Method::POST,
        "/api/v1/telemetry/events",
        Some(&token),
        Some(telemetry_event(
            selected_id,
            session_id,
            "game_selected",
            &[],
        )),
    )
    .await;
    assert_eq!(duplicate_status, StatusCode::OK);
    assert_eq!(duplicate.payload["duplicate"], true);

    for invalid in [
        json!({
            "event_id": "not-a-uuid",
            "anonymous_session_id": session_id,
            "event": "game_selected",
            "game_id": "sfiii3",
            "blocked_checks": []
        }),
        telemetry_event(
            uuid::Uuid::new_v4(),
            session_id,
            "readiness_completed",
            &["desktop"],
        ),
        telemetry_event(
            uuid::Uuid::new_v4(),
            session_id,
            "readiness_blocked",
            &["network"],
        ),
    ] {
        let (status, _) = request(
            &app,
            Method::POST,
            "/api/v1/telemetry/events",
            Some(&token),
            Some(invalid),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    for _ in 0..3 {
        let (status, _) = request(
            &app,
            Method::POST,
            "/api/v1/telemetry/events",
            Some(&token),
            Some(telemetry_event(
                uuid::Uuid::new_v4(),
                session_id,
                "readiness_blocked",
                &["game_runtime"],
            )),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
    }

    let expired_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO product_events
            (event_id, anonymous_session_id, event_name, game_id, received_at)
         VALUES ($1, $2, 'game_selected', 'sfiii3', now() - interval '91 days')",
    )
    .bind(expired_id)
    .bind(uuid::Uuid::new_v4())
    .execute(&pool)
    .await
    .expect("insert expired telemetry fixture");
    let (status, _) = request(
        &app,
        Method::POST,
        "/api/v1/telemetry/events",
        Some(&token),
        Some(telemetry_event(
            uuid::Uuid::new_v4(),
            session_id,
            "game_selected",
            &[],
        )),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM product_events WHERE event_id = $1")
            .bind(expired_id)
            .fetch_one(&pool)
            .await
            .expect("count expired event"),
        0
    );

    let (summary_status, summary) = operator_request(
        &app,
        Method::GET,
        "/api/v1/telemetry/activation",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(summary_status, StatusCode::OK);
    assert_eq!(summary.payload["window_days"], 30);
    assert_eq!(summary.payload["selected_sessions"], 3);
    assert_eq!(summary.payload["ready_sessions"], 3);
    assert_eq!(summary.payload["lobby_sessions"], 3);
    assert_eq!(summary.payload["launch_attempted_sessions"], 0);
    assert_eq!(summary.payload["launch_succeeded_sessions"], 0);
    assert_eq!(summary.payload["readiness_blocked_events"], 3);
    assert_eq!(summary.payload["selected_to_ready_rate"], 1.0);
    assert_eq!(summary.payload["selected_to_lobby_rate"], 1.0);
    assert_eq!(summary.payload["selected_to_launch_rate"], 0.0);
    assert_eq!(
        summary.payload["blocked_by_check"][0]["check"],
        "game_runtime"
    );
    assert_eq!(summary.payload["blocked_by_check"][0]["count"], 3);

    let stored_columns: Vec<String> = sqlx::query_scalar(
        "SELECT column_name FROM information_schema.columns
         WHERE table_name = 'product_events' ORDER BY ordinal_position",
    )
    .fetch_all(&pool)
    .await
    .expect("inspect telemetry schema");
    assert!(!stored_columns.iter().any(|column| column == "user_id"));
}
