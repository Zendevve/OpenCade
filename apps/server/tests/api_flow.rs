use axum::{
    body::{to_bytes, Body},
    http::{header, Method, Request, StatusCode},
    Router,
};
use futures_util::{SinkExt, StreamExt};
use opencade_protocol::Envelope;
use opencade_server::{build_app, AppState, Config};
use serde_json::{json, Value};
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
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
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

#[sqlx::test]
async fn authenticated_users_create_and_accept_a_room(pool: PgPool) {
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations should succeed");
    let app = build_app(AppState::new(pool.clone(), Config::for_test()));
    let host_token = register(&app, "host_player").await;
    let guest_token = register(&app, "guest_player").await;
    let observer_token = register(&app, "observer_player").await;
    let guest_id = current_user_id(&app, &guest_token).await;

    let (games_status, games) =
        request(&app, Method::GET, "/api/v1/games", Some(&host_token), None).await;
    assert_eq!(games_status, StatusCode::OK);
    assert_eq!(games.payload["games"].as_array().map(Vec::len), Some(5));

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
        .send(Message::Text(offer.to_string()))
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

    let (start_status, started) = request(
        &app,
        Method::POST,
        &format!("/api/v1/rooms/{room_id}/start"),
        Some(&host_token),
        None,
    )
    .await;
    assert_eq!(start_status, StatusCode::OK);
    assert_eq!(started.payload["state"], "playing");
    let (finish_status, finished) = request(
        &app,
        Method::POST,
        &format!("/api/v1/rooms/{room_id}/finish"),
        Some(&guest_token),
        None,
    )
    .await;
    assert_eq!(finish_status, StatusCode::OK);
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
