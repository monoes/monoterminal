use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;

use crate::protocol::{ClientMessage, ServerMessage};
use crate::state::AppState;
use crate::SharedState;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(shared): State<Arc<SharedState>>,
) -> impl IntoResponse {
    let state = shared.relay.clone();
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let conn_id = state.next_conn_id();
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Message>();

    state.add_connection(conn_id, out_tx).await;

    let send_task = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if ws_tx.send(msg).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(msg)) = ws_rx.next().await {
        let text = match msg {
            Message::Text(text) => text,
            Message::Close(_) => break,
            _ => continue,
        };

        handle_text_message(&state, conn_id, &text).await;
    }

    if let Some(peer_conn_id) = state.remove_connection(conn_id).await {
        send_json(&state, peer_conn_id, &ServerMessage::PeerDisconnected).await;
    }

    send_task.abort();
}

async fn handle_text_message(state: &Arc<AppState>, conn_id: u64, text: &str) {
    let parsed: ClientMessage = match serde_json::from_str(text) {
        Ok(msg) => msg,
        Err(err) => {
            tracing::warn!(conn_id, error = %err, "ignoring malformed message");
            return;
        }
    };

    match parsed {
        ClientMessage::Register { peer_id, handshake } => {
            let handshake_peer_id = handshake.get("peer_id").and_then(|v| v.as_str());
            if handshake_peer_id != Some(peer_id.as_str()) {
                send_json(
                    state,
                    conn_id,
                    &ServerMessage::Error {
                        message: "handshake.peer_id does not match peer_id".to_string(),
                    },
                )
                .await;
                return;
            }

            state.register(conn_id, peer_id.clone()).await;
            tracing::info!(conn_id, peer_id = %peer_id, "daemon registered");
            send_json(state, conn_id, &ServerMessage::Registered).await;
        }

        ClientMessage::Connect { peer_id } => match state.connect(conn_id, &peer_id).await {
            Some(daemon_conn_id) => {
                tracing::info!(conn_id, peer_id = %peer_id, "browser paired with daemon");
                send_json(state, conn_id, &ServerMessage::Connected).await;
                send_json(state, daemon_conn_id, &ServerMessage::PeerConnectRequest).await;
            }
            None => {
                send_json(
                    state,
                    conn_id,
                    &ServerMessage::Error {
                        message: "peer not found or offline".to_string(),
                    },
                )
                .await;
            }
        },

        ClientMessage::Offer { .. } | ClientMessage::Answer { .. } | ClientMessage::IceCandidate { .. } => {
            forward_raw(state, conn_id, text).await;
        }
    }
}

/// Forwards the raw text frame verbatim to whichever socket `conn_id` is
/// currently paired with.
async fn forward_raw(state: &Arc<AppState>, conn_id: u64, text: &str) {
    match state.paired_with(conn_id).await {
        Some(other_id) => {
            state
                .send_to(other_id, Message::Text(text.to_string()))
                .await;
        }
        None => {
            tracing::warn!(conn_id, "no pair to forward message to");
        }
    }
}

async fn send_json(state: &Arc<AppState>, conn_id: u64, message: &ServerMessage) {
    match serde_json::to_string(message) {
        Ok(text) => {
            state.send_to(conn_id, Message::Text(text)).await;
        }
        Err(err) => {
            tracing::error!(error = %err, "failed to serialize outgoing message");
        }
    }
}
