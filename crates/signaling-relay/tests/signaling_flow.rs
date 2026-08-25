use futures_util::{SinkExt, StreamExt};
use monoterminal_signaling_relay::build_router;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

async fn spawn_server() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = build_router();

    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    format!("ws://{}/", addr)
}

async fn recv_json(ws: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin)) -> serde_json::Value {
    loop {
        match ws.next().await.expect("stream closed").expect("ws error") {
            Message::Text(text) => return serde_json::from_str(&text).unwrap(),
            _ => continue,
        }
    }
}

#[tokio::test]
async fn register_connect_and_forward_offer() {
    let url = spawn_server().await;

    let (mut daemon, _) = connect_async(&url).await.expect("daemon connect");
    let (mut browser, _) = connect_async(&url).await.expect("browser connect");

    let peer_id = "deadbeefcafef00d";

    daemon
        .send(Message::Text(
            serde_json::json!({
                "type": "register",
                "peer_id": peer_id,
                "handshake": {
                    "protocol_version": 1u32,
                    "peer_id": peer_id,
                    "timestamp_ms": 1_700_000_000_000u64,
                    "signature": [1, 2, 3, 4]
                }
            })
            .to_string(),
        ))
        .await
        .unwrap();

    let registered = recv_json(&mut daemon).await;
    assert_eq!(registered["type"], "registered");

    browser
        .send(Message::Text(
            serde_json::json!({"type": "connect", "peer_id": peer_id}).to_string(),
        ))
        .await
        .unwrap();

    let connected = recv_json(&mut browser).await;
    assert_eq!(connected["type"], "connected");

    let peer_connect_request = recv_json(&mut daemon).await;
    assert_eq!(peer_connect_request["type"], "peer_connect_request");

    // Daemon sends an offer; it must arrive on the browser side, not echo
    // back to the daemon itself.
    daemon
        .send(Message::Text(
            serde_json::json!({"type": "offer", "sdp": "v=0 fake-sdp"}).to_string(),
        ))
        .await
        .unwrap();

    let offer = recv_json(&mut browser).await;
    assert_eq!(offer["type"], "offer");
    assert_eq!(offer["sdp"], "v=0 fake-sdp");

    // Browser answers back.
    browser
        .send(Message::Text(
            serde_json::json!({"type": "answer", "sdp": "v=0 fake-answer"}).to_string(),
        ))
        .await
        .unwrap();

    let answer = recv_json(&mut daemon).await;
    assert_eq!(answer["type"], "answer");
    assert_eq!(answer["sdp"], "v=0 fake-answer");

    // ICE candidate forwarding.
    daemon
        .send(Message::Text(
            serde_json::json!({
                "type": "ice_candidate",
                "candidate": "candidate:1 1 UDP 1 127.0.0.1 1234 typ host",
                "sdp_mid": "0",
                "sdp_mline_index": 0
            })
            .to_string(),
        ))
        .await
        .unwrap();

    let ice = recv_json(&mut browser).await;
    assert_eq!(ice["type"], "ice_candidate");

    // Daemon disconnects -> browser should be told the peer disconnected.
    daemon.close(None).await.unwrap();

    let disconnected = recv_json(&mut browser).await;
    assert_eq!(disconnected["type"], "peer_disconnected");
}

#[tokio::test]
async fn connect_to_unknown_peer_gets_error() {
    let url = spawn_server().await;
    let (mut browser, _) = connect_async(&url).await.expect("browser connect");

    browser
        .send(Message::Text(
            serde_json::json!({"type": "connect", "peer_id": "nonexistent"}).to_string(),
        ))
        .await
        .unwrap();

    let error = recv_json(&mut browser).await;
    assert_eq!(error["type"], "error");
    assert_eq!(error["message"], "peer not found or offline");
}

#[tokio::test]
async fn malformed_message_is_ignored_not_fatal() {
    let url = spawn_server().await;
    let (mut client, _) = connect_async(&url).await.expect("client connect");

    client
        .send(Message::Text("not json at all".to_string()))
        .await
        .unwrap();

    // Connection should remain usable afterwards.
    client
        .send(Message::Text(
            serde_json::json!({"type": "connect", "peer_id": "whatever"}).to_string(),
        ))
        .await
        .unwrap();

    let error = recv_json(&mut client).await;
    assert_eq!(error["type"], "error");
}
