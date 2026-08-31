// Signaling relay client — the daemon's half of the WebRTC P2P remote-access
// path (see docs/decisions/011-p2p-networking-architecture.md and the plan
// this implements, "WebRTC P2P remote access — Phase 1 (STUN-only)").
//
// The relay (crates/signaling-relay) never touches the monoterminal
// protobuf protocol — it only shuffles plain JSON (SDP offer/answer, ICE
// candidates) between exactly two paired sockets so a browser and this
// daemon can establish a direct WebRTC DataChannel. Once that channel is
// open, terminal traffic flows over it using the exact same
// `process_message` used by the direct-WebSocket path
// (`server::handler::handle_datachannel_session`) — this module only
// handles getting that channel open in the first place.

use std::sync::Arc;
use std::time::Duration;

use ed25519_dalek::SigningKey;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tracing::{debug, error, info, warn};

use crate::auth::AuthService;
use crate::clipboard::ClipboardManager;
use crate::server::handler::handle_datachannel_session;
use crate::session::manager::SessionManager;
use crate::webrtc::config::WebRtcConfig;
use crate::webrtc::pairing::PairingCodeCache;
use crate::webrtc::handshake::PeerHandshake;
use crate::webrtc::ice::IceCandidate;
use crate::webrtc::peer_connection::{DataChannelMessage, PeerConnection, PeerConnectionState};

/// Wire protocol spoken with the signaling relay. Plain JSON, one object per
/// text frame — deliberately decoupled from the protobuf `Envelope` used
/// once the DataChannel is up.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RelayMessage {
    Register {
        peer_id: String,
        handshake: PeerHandshake,
    },
    Registered,
    Connect {
        peer_id: String,
    },
    Connected,
    PeerConnectRequest,
    Offer {
        sdp: String,
    },
    Answer {
        sdp: String,
    },
    IceCandidate {
        candidate: String,
        sdp_mid: Option<String>,
        sdp_mline_index: Option<u16>,
    },
    PeerDisconnected,
    Error {
        message: String,
    },
}

impl From<IceCandidate> for RelayMessage {
    fn from(c: IceCandidate) -> Self {
        RelayMessage::IceCandidate {
            candidate: c.candidate,
            sdp_mid: c.sdp_mid,
            sdp_mline_index: c.sdp_mline_index,
        }
    }
}

/// An in-progress (or freshly-connected) P2P negotiation with a single
/// remote peer. Phase 1 handles one active negotiation at a time — a second
/// `PeerConnectRequest` while one is already connecting replaces it.
struct Negotiation {
    pc: Arc<PeerConnection>,
    ice_rx: mpsc::Receiver<IceCandidate>,
    /// Taken once the DataChannel opens and its session loop is spawned.
    messages_rx: Option<mpsc::Receiver<DataChannelMessage>>,
}

/// Runs forever, reconnecting to the relay with backoff on any error.
/// Intended to be spawned as a background task alongside the main
/// WebSocket server (see `main.rs`, `--enable-p2p`/`--relay-url`).
pub async fn run(
    relay_url: String,
    signing_key: Arc<SigningKey>,
    session_manager: Arc<SessionManager>,
    clipboard_manager: Arc<ClipboardManager>,
    auth_service: Arc<dyn AuthService>,
    dev_mode: bool,
    pairing_cache: Option<Arc<PairingCodeCache>>,
) {
    let peer_id = hex::encode(signing_key.verifying_key().to_bytes());
    info!("P2P signaling client starting (peer_id: {})", peer_id);

    loop {
        match run_once(
            &relay_url,
            &peer_id,
            &signing_key,
            &session_manager,
            &clipboard_manager,
            &auth_service,
            dev_mode,
            pairing_cache.clone(),
        )
        .await
        {
            Ok(()) => warn!("Signaling relay connection closed, reconnecting in 5s"),
            Err(e) => warn!("Signaling relay connection error: {} — retrying in 5s", e),
        }

        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

async fn run_once(
    relay_url: &str,
    peer_id: &str,
    signing_key: &SigningKey,
    session_manager: &Arc<SessionManager>,
    clipboard_manager: &Arc<ClipboardManager>,
    auth_service: &Arc<dyn AuthService>,
    dev_mode: bool,
    pairing_cache: Option<Arc<PairingCodeCache>>,
) -> anyhow::Result<()> {
    let (ws_stream, _) = tokio_tungstenite::connect_async(relay_url).await?;
    let (mut write, mut read) = ws_stream.split();

    let handshake = PeerHandshake::new(signing_key)?;
    let register = RelayMessage::Register {
        peer_id: peer_id.to_string(),
        handshake,
    };
    write
        .send(WsMessage::Text(serde_json::to_string(&register)?))
        .await?;

    let mut negotiation: Option<Negotiation> = None;

    // Detects a "zombie" relay connection: the TCP socket can go dead
    // (network blip, idle proxy timeout, sleep/wake) without either side
    // ever observing a close or a read error, leaving the daemon
    // "registered" against a link that no longer carries traffic — the
    // relay then reports any browser's Connect as paired, but nothing the
    // daemon sends ever arrives. Pinging on an interval and tracking the
    // last time *anything* was received (including the relay's own
    // pings/pongs) lets a stale connection be detected and torn down so
    // the outer reconnect loop in `run()` can establish a fresh one,
    // instead of requiring a manual daemon restart.
    const PING_INTERVAL: Duration = Duration::from_secs(15);
    const STALE_TIMEOUT: Duration = Duration::from_secs(45);
    let mut last_activity = tokio::time::Instant::now();
    let mut ping_interval = tokio::time::interval(PING_INTERVAL);

    loop {
        tokio::select! {
            // Messages from the relay
            msg = read.next() => {
                let msg = match msg {
                    Some(Ok(m)) => m,
                    Some(Err(e)) => return Err(anyhow::anyhow!("relay socket error: {}", e)),
                    None => return Ok(()), // relay closed the connection
                };
                last_activity = tokio::time::Instant::now();

                let text = match msg {
                    WsMessage::Text(t) => t,
                    WsMessage::Close(_) => return Ok(()),
                    WsMessage::Ping(payload) => {
                        // tungstenite doesn't auto-reply on a split stream —
                        // reply ourselves so the relay (or any proxy in
                        // front of it) sees this connection as alive.
                        let _ = write.send(WsMessage::Pong(payload)).await;
                        continue;
                    }
                    _ => continue, // ignore pong/binary — relay only speaks JSON text
                };

                let relay_msg: RelayMessage = match serde_json::from_str(&text) {
                    Ok(m) => m,
                    Err(e) => {
                        warn!("Ignoring malformed message from relay: {}", e);
                        continue;
                    }
                };

                match relay_msg {
                    RelayMessage::Registered => info!("Registered with signaling relay"),
                    RelayMessage::Error { message } => warn!("Relay error: {}", message),
                    RelayMessage::Connected
                    | RelayMessage::Connect { .. }
                    | RelayMessage::Register { .. }
                    | RelayMessage::Answer { .. } => {
                        // These are messages the daemon only ever sends (Register)
                        // or that are meant for the browser side (Connect/Connected/
                        // Answer) — receiving one back would indicate a relay bug.
                        // Ignore defensively rather than crash.
                        debug!("Ignoring unexpected relay message: {:?}", relay_msg);
                    }

                    RelayMessage::PeerConnectRequest => {
                        info!("Incoming P2P connection request");
                        // Best-effort: if the relay's TURN endpoint is
                        // unreachable, fall back to STUN-only rather than
                        // failing the connection outright — matches today's
                        // behavior exactly when TURN isn't configured.
                        let turn_servers =
                            match crate::webrtc::turn::fetch_turn_credentials(relay_url, peer_id)
                                .await
                            {
                                Ok(turn) => Some(turn),
                                Err(e) => {
                                    warn!("Failed to fetch TURN credentials, falling back to STUN-only: {}", e);
                                    None
                                }
                            };
                        let config = Arc::new(WebRtcConfig {
                            turn_servers,
                            ..Default::default()
                        });
                        match PeerConnection::new_as_answerer(config).await {
                            Ok((pc, ice_rx, messages_rx)) => {
                                negotiation = Some(Negotiation {
                                    pc: Arc::new(pc),
                                    ice_rx,
                                    messages_rx: Some(messages_rx),
                                });
                            }
                            Err(e) => error!("Failed to create answerer PeerConnection: {}", e),
                        }
                    }

                    RelayMessage::Offer { sdp } => {
                        let Some(neg) = negotiation.as_ref() else {
                            warn!("Received offer with no pending negotiation, ignoring");
                            continue;
                        };
                        if let Err(e) = neg.pc.set_remote_offer(sdp).await {
                            error!("Failed to set remote offer: {}", e);
                            continue;
                        }
                        match neg.pc.create_answer().await {
                            Ok(answer_sdp) => {
                                let msg = RelayMessage::Answer { sdp: answer_sdp };
                                write.send(WsMessage::Text(serde_json::to_string(&msg)?)).await?;
                            }
                            Err(e) => error!("Failed to create answer: {}", e),
                        }
                    }

                    RelayMessage::IceCandidate { candidate, sdp_mid, sdp_mline_index } => {
                        let Some(neg) = negotiation.as_ref() else {
                            debug!("Received ICE candidate with no pending negotiation, ignoring");
                            continue;
                        };
                        let ice = IceCandidate {
                            candidate,
                            sdp_mid,
                            sdp_mline_index,
                            username_fragment: None,
                        };
                        if let Err(e) = neg.pc.add_ice_candidate(ice).await {
                            warn!("Failed to add ICE candidate: {}", e);
                        }
                    }

                    RelayMessage::PeerDisconnected => {
                        // The browser closes its signaling-relay socket the
                        // instant its DataChannel opens (the relay's job is
                        // done once P2P takes over) — the relay reports
                        // that same-socket closure as "peer disconnected"
                        // regardless of whether negotiation succeeded. Only
                        // tear down the PeerConnection if it never actually
                        // reached Connected; otherwise this races the just
                        // established DataChannel session and kills it.
                        let already_connected = match &negotiation {
                            Some(neg) => neg.pc.state().await == PeerConnectionState::Connected,
                            None => false,
                        };
                        if already_connected {
                            debug!("Signaling relay disconnected after successful negotiation — ignoring");
                        } else {
                            info!("Remote peer disconnected before negotiation completed");
                            if let Some(neg) = negotiation.take() {
                                let _ = neg.pc.close().await;
                            }
                        }
                    }
                }
            }

            // Our own trickle-ICE candidates for the active negotiation
            Some(candidate) = async {
                match &mut negotiation {
                    Some(neg) => neg.ice_rx.recv().await,
                    None => std::future::pending().await,
                }
            } => {
                let msg: RelayMessage = candidate.into();
                write.send(WsMessage::Text(serde_json::to_string(&msg)?)).await?;
            }

            // Keepalive: probe the connection on an interval, and give up
            // on it (triggering a reconnect) if nothing at all has been
            // heard back since well before the last couple of probes.
            _ = ping_interval.tick() => {
                if last_activity.elapsed() > STALE_TIMEOUT {
                    return Err(anyhow::anyhow!(
                        "relay connection stale — no activity in {}s, reconnecting",
                        last_activity.elapsed().as_secs()
                    ));
                }
                if let Err(e) = write.send(WsMessage::Ping(Vec::new())).await {
                    return Err(anyhow::anyhow!("failed to send keepalive ping: {}", e));
                }
            }
        }

        // Once the DataChannel opens, hand off to the shared protocol
        // session loop and stop tracking this negotiation here (it lives on
        // inside the spawned task via the Arc<PeerConnection> clone).
        if let Some(neg) = &mut negotiation {
            if neg.messages_rx.is_some() && neg.pc.state().await == PeerConnectionState::Connected {
                let messages_rx = neg.messages_rx.take().expect("checked is_some above");
                let pc = neg.pc.clone();
                let sm = session_manager.clone();
                let cm = clipboard_manager.clone();
                let auth = auth_service.clone();
                let pairing = pairing_cache.clone();
                info!("P2P DataChannel open — starting terminal session over it");
                tokio::spawn(async move {
                    handle_datachannel_session(pc, messages_rx, sm, cm, auth, dev_mode, pairing)
                        .await;
                });
            }
        }
    }
}
