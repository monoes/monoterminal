// WebSocket message handler with Protocol Buffer integration
// Implements task-3: Protocol Runtime Integration
// Implements task-2: Monomind bridge integration (health/upgrade/detection/dashboard)
// Implements task-8: JWT authentication integration

#![allow(clippy::too_many_arguments)]

use futures_util::{SinkExt, StreamExt};
use prost::Message;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_rustls::server::TlsStream;
use tokio_tungstenite::tungstenite::protocol::Message as WsMessage;
use tokio_tungstenite::WebSocketStream;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::error::{Result, ServerError};
use crate::auth::{AuthService, Claims};
use crate::clipboard::ClipboardManager;
use crate::session::manager::SessionManager;
use crate::session::{ClientId, SessionId};
use crate::webrtc::PairingCodeCache;
use monoterminal_protocol::{envelope, Envelope, ErrorCode};

/// Handle WebSocket connection with bidirectional streaming
/// Client → Server: AttachRequest, InputData, ResizeRequest, DetachRequest, ClipboardGetRequest, ClipboardSetRequest
/// Server → Client: AttachResponse, OutputData (continuous), ErrorResponse, ClipboardGetResponse
pub async fn handle_websocket(
    ws_stream: WebSocketStream<TlsStream<TcpStream>>,
    peer_addr: SocketAddr,
    session_manager: Arc<SessionManager>,
    clipboard_manager: Arc<ClipboardManager>,
    auth_service: Arc<dyn AuthService>,
    dev_mode: bool,
    pairing_cache: Option<Arc<PairingCodeCache>>,
) -> Result<()> {
    let (mut ws_write, mut ws_read) = ws_stream.split();
    let mut sequence_number: u64 = 0;
    let client_id = Uuid::new_v4();

    info!(
        "WebSocket handler started for {} (client_id: {})",
        peer_addr, client_id
    );

    // Connection state
    let mut attached_session: Option<SessionId> = None;
    let mut output_rx: Option<mpsc::Receiver<Vec<u8>>> = None;
    let mut conn_output_tx: Option<mpsc::Sender<Vec<u8>>> = None;

    // Main message loop
    loop {
        tokio::select! {
            // Receive output from PTY (if attached)
            Some(output_data) = async {
                match &mut output_rx {
                    Some(rx) => rx.recv().await,
                    None => None,
                }
            } => {
                // Send output to client
                if let Err(e) = ws_write.send(WsMessage::Binary(output_data)).await {
                    error!("Failed to send output to {}: {}", peer_addr, e);
                    break;
                }
            }

            // Receive message from client
            Some(msg) = ws_read.next() => {
                match msg {
                    Ok(WsMessage::Binary(data)) => {
                        debug!("Received binary message from {} ({} bytes)", peer_addr, data.len());

                        // Decode Protocol Buffer Envelope
                        match Envelope::decode(&data[..]) {
                            Ok(envelope) => {
                                debug!("Decoded envelope with sequence {}", envelope.sequence_number);

                                // Process message
                                match process_message(
                                    envelope,
                                    &session_manager,
                                    &clipboard_manager,
                                    auth_service.as_ref(),
                                    dev_mode,
                                    client_id,
                                    &mut attached_session,
                                    &mut output_rx,
                                    &mut conn_output_tx,
                                    peer_addr,
                                    pairing_cache.as_ref(),
                                ).await {
                                    Ok(Some(response)) => {
                                        // Encode and send response
                                        let mut response_bytes = Vec::with_capacity(response.encoded_len());
                                        if let Err(e) = response.encode(&mut response_bytes) {
                                            error!("Failed to encode response: {}", e);
                                            continue;
                                        }

                                        if let Err(e) = ws_write.send(WsMessage::Binary(response_bytes)).await {
                                            error!("Failed to send response to {}: {}", peer_addr, e);
                                            break;
                                        }

                                        sequence_number += 1;
                                    }
                                    Ok(None) => {
                                        // No response needed (e.g., InputData)
                                        debug!("Message processed, no response needed");
                                    }
                                    Err(e) => {
                                        error!("Failed to process message: {}", e);

                                        // Send error response
                                        let error_response = create_error_envelope(sequence_number, e);
                                        let mut error_bytes = Vec::with_capacity(error_response.encoded_len());
                                        if let Err(e) = error_response.encode(&mut error_bytes) {
                                            error!("Failed to encode error response: {}", e);
                                            break;
                                        }

                                        if let Err(e) = ws_write.send(WsMessage::Binary(error_bytes)).await {
                                            error!("Failed to send error response to {}: {}", peer_addr, e);
                                            break;
                                        }

                                        sequence_number += 1;
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Failed to decode Protocol Buffer message from {}: {}", peer_addr, e);

                                // Send protocol error
                                let error_response = Envelope {
                                    sequence_number,
                                    message: Some(envelope::Message::ErrorResponse(
                                        monoterminal_protocol::ErrorResponse {
                                            code: monoterminal_protocol::ErrorCode::InvalidRequest as i32,
                                            message: format!("Protocol decode error: {}", e),
                                        }
                                    )),
                                };

                                let mut error_bytes = Vec::with_capacity(error_response.encoded_len());
                                if let Err(e) = error_response.encode(&mut error_bytes) {
                                    error!("Failed to encode error response: {}", e);
                                    break;
                                }

                                if let Err(e) = ws_write.send(WsMessage::Binary(error_bytes)).await {
                                    error!("Failed to send error response to {}: {}", peer_addr, e);
                                    break;
                                }

                                sequence_number += 1;
                            }
                        }
                    }
                    Ok(WsMessage::Text(text)) => {
                        warn!("Received unexpected text message from {}: {}", peer_addr, text);
                    }
                    Ok(WsMessage::Ping(data)) => {
                        debug!("Received ping from {}", peer_addr);
                        if let Err(e) = ws_write.send(WsMessage::Pong(data)).await {
                            error!("Failed to send pong to {}: {}", peer_addr, e);
                            break;
                        }
                    }
                    Ok(WsMessage::Pong(_)) => {
                        debug!("Received pong from {}", peer_addr);
                    }
                    Ok(WsMessage::Close(frame)) => {
                        info!("Client {} closed connection: {:?}", peer_addr, frame);
                        break;
                    }
                    Ok(WsMessage::Frame(_)) => {
                        warn!("Received raw frame from {}", peer_addr);
                    }
                    Err(e) => {
                        error!("WebSocket error from {}: {}", peer_addr, e);
                        break;
                    }
                }
            }

            else => {
                // Both channels closed
                debug!("Both WebSocket channels closed for {}", peer_addr);
                break;
            }
        }
    }

    // Cleanup: detach from session if attached
    if let Some(session_id) = attached_session {
        if let Err(e) = session_manager.detach_client(session_id, client_id).await {
            error!(
                "Failed to detach client {} from session {}: {}",
                client_id, session_id, e
            );
        } else {
            info!(
                "Client {} detached from session {} on disconnect",
                client_id, session_id
            );
        }
    }

    info!("WebSocket handler stopped for {}", peer_addr);
    Ok(())
}

/// Handle a WebRTC DataChannel connection with the exact same protocol
/// processing as `handle_websocket` — only the transport differs. The
/// DataChannel is negotiated out-of-band via the signaling relay
/// (`crate::webrtc::signaling_client`); once open, terminal traffic
/// (Attach/Input/Output/Resize/...) flows over it using the same protobuf
/// `Envelope` encode/decode and the same `process_message` used by the
/// direct-WebSocket path, so all session/auth/RBAC logic stays shared.
pub async fn handle_datachannel_session(
    peer_connection: Arc<crate::webrtc::PeerConnection>,
    mut messages_rx: mpsc::Receiver<crate::webrtc::peer_connection::DataChannelMessage>,
    session_manager: Arc<SessionManager>,
    clipboard_manager: Arc<ClipboardManager>,
    auth_service: Arc<dyn AuthService>,
    dev_mode: bool,
    pairing_cache: Option<Arc<PairingCodeCache>>,
) {
    // P2P connections have no socket peer address — process_message only
    // uses this for logging, so a placeholder is fine here.
    let peer_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let client_id = Uuid::new_v4();
    let mut sequence_number: u64 = 0;

    info!("DataChannel session started (client_id: {})", client_id);

    let mut attached_session: Option<SessionId> = None;
    let mut output_rx: Option<mpsc::Receiver<Vec<u8>>> = None;
    let mut conn_output_tx: Option<mpsc::Sender<Vec<u8>>> = None;

    loop {
        tokio::select! {
            // Receive output from PTY (if attached)
            Some(output_data) = async {
                match &mut output_rx {
                    Some(rx) => rx.recv().await,
                    None => None,
                }
            } => {
                if let Err(e) = peer_connection.send(&output_data).await {
                    error!("Failed to send output over DataChannel: {}", e);
                    break;
                }
            }

            // Receive message from the DataChannel
            Some(dc_msg) = messages_rx.recv() => {
                match Envelope::decode(&dc_msg.data[..]) {
                    Ok(envelope) => {
                        match process_message(
                            envelope,
                            &session_manager,
                            &clipboard_manager,
                            auth_service.as_ref(),
                            dev_mode,
                            client_id,
                            &mut attached_session,
                            &mut output_rx,
                            &mut conn_output_tx,
                            peer_addr,
                            pairing_cache.as_ref(),
                        ).await {
                            Ok(Some(response)) => {
                                let mut response_bytes = Vec::with_capacity(response.encoded_len());
                                if let Err(e) = response.encode(&mut response_bytes) {
                                    error!("Failed to encode DataChannel response: {}", e);
                                    continue;
                                }

                                if let Err(e) = peer_connection.send(&response_bytes).await {
                                    error!("Failed to send DataChannel response: {}", e);
                                    break;
                                }

                                sequence_number += 1;
                            }
                            Ok(None) => {
                                debug!("DataChannel message processed, no response needed");
                            }
                            Err(e) => {
                                error!("Failed to process DataChannel message: {}", e);

                                let error_response = create_error_envelope(sequence_number, e);
                                let mut error_bytes = Vec::with_capacity(error_response.encoded_len());
                                if error_response.encode(&mut error_bytes).is_ok()
                                    && peer_connection.send(&error_bytes).await.is_err()
                                {
                                    error!("Failed to send DataChannel error response");
                                    break;
                                }

                                sequence_number += 1;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to decode Protocol Buffer message from DataChannel: {}", e);
                    }
                }
            }

            else => {
                debug!("DataChannel session channels closed (client_id: {})", client_id);
                break;
            }
        }
    }

    if let Some(session_id) = attached_session {
        if let Err(e) = session_manager.detach_client(session_id, client_id).await {
            error!(
                "Failed to detach P2P client {} from session {}: {}",
                client_id, session_id, e
            );
        } else {
            info!(
                "P2P client {} detached from session {} on disconnect",
                client_id, session_id
            );
        }
    }

    info!("DataChannel session stopped (client_id: {})", client_id);
}

/// Verify JWT authentication token
///
/// SRS §3.2.2: Ed25519/JWT authentication with 15-minute access tokens
///
/// # Arguments
/// * `auth_service` - The authentication service to verify tokens
/// * `token` - The JWT token string (EdDSA signed)
///
/// # Returns
/// * `Ok(Claims)` - Valid token with user claims
/// * `Err(ServerError::AuthFailed)` - Invalid, expired, or malformed token
fn verify_auth_token(auth_service: &dyn AuthService, token: &str) -> Result<Claims> {
    auth_service
        .verify_access(token)
        .map_err(|e| ServerError::AuthFailed(format!("JWT verification failed: {}", e)))
}

/// Process a Protocol Buffer message
async fn process_message(
    envelope: Envelope,
    session_manager: &SessionManager,
    clipboard_manager: &ClipboardManager,
    auth_service: &dyn AuthService,
    dev_mode: bool,
    client_id: ClientId,
    attached_session: &mut Option<SessionId>,
    output_rx: &mut Option<mpsc::Receiver<Vec<u8>>>,
    // Kept alongside `output_rx` (not just used once at attach time) so that
    // splitting a pane can attach the SAME connection to the new pane's
    // session too — every pane's output then arrives pre-tagged with its
    // pane_id (see OutputData.pane_id) over this one shared channel, without
    // needing any dynamic multi-stream fan-in machinery in the select loop.
    conn_output_tx: &mut Option<mpsc::Sender<Vec<u8>>>,
    peer_addr: SocketAddr,
    pairing_cache: Option<&Arc<PairingCodeCache>>,
) -> Result<Option<Envelope>> {
    match envelope.message {
        Some(envelope::Message::AttachRequest(req)) => {
            debug!(
                "Processing AttachRequest from {}: session_id={}",
                peer_addr, req.session_id
            );

            // SRS §3.2.2: JWT authentication verification
            // Phase 2: Extract user_id from JWT claims for RBAC
            let user_id = if !dev_mode {
                // Production mode: verify JWT token and extract user_id
                if req.auth_token.is_empty() {
                    warn!("AttachRequest from {} missing auth_token", peer_addr);
                    return Err(ServerError::AuthFailed(
                        "Missing authentication token".to_string(),
                    ));
                }

                let claims = verify_auth_token(auth_service, &req.auth_token)?;
                debug!(
                    "JWT verified for AttachRequest from {}: user_id={}",
                    peer_addr, claims.sub
                );
                Some(claims.sub)
            } else {
                // Dev mode: bypass auth (for E2E testing only)
                warn!(
                    "⚠️  DEV MODE: Skipping JWT verification for AttachRequest from {}",
                    peer_addr
                );
                None
            };

            // Parse or create session_id (protocol: "UUID or empty for new session")
            let session_id = if !req.session_id.is_empty() {
                // Attach to existing session
                Uuid::parse_str(&req.session_id).map_err(|e| {
                    ServerError::InvalidMessage(format!("Invalid session_id UUID: {}", e))
                })?
            } else if !req.session_name.is_empty() {
                // Find-or-create by stable logical key, so multiple clients
                // referring to the same logical terminal converge on one session
                info!(
                    "Resolving named session '{}' for {} ({}x{}, user_id={:?})",
                    req.session_name, peer_addr, req.rows, req.cols, user_id
                );
                session_manager
                    .resolve_named_session(
                        &req.session_name,
                        user_id.clone(),
                        req.rows as u16,
                        req.cols as u16,
                    )
                    .await
                    .map_err(|e| {
                        ServerError::InvalidMessage(format!("Failed to resolve session: {}", e))
                    })?
            } else {
                // Create new session with requested dimensions (Phase 2: set owner from JWT)
                info!(
                    "Creating new session for {} ({}x{}, user_id={:?})",
                    peer_addr, req.rows, req.cols, user_id
                );
                session_manager
                    .create_session_with_user(
                        user_id.clone(),
                        None,
                        req.rows as u16,
                        req.cols as u16,
                    )
                    .await
                    .map_err(|e| {
                        ServerError::InvalidMessage(format!("Failed to create session: {}", e))
                    })?
            };

            // Create output channel for this client
            let (client_output_tx, rx) = mpsc::channel(256); // 256 messages ≈ 1MB buffer per SRS §3.1.4

            // Attach client to session (Phase 2: RBAC permission check)
            let snapshot = session_manager
                .attach_client_with_user(session_id, client_id, client_output_tx.clone(), user_id.clone())
                .await
                .map_err(|e| match e {
                    crate::session::SessionError::NotFound(_) => {
                        ServerError::SessionNotFound(req.session_id.clone())
                    }
                    _ => ServerError::InvalidMessage(format!("Attach failed: {}", e)),
                })?;

            // Store connection state. `conn_output_tx` is kept (not just used
            // once here) so that a later SplitPaneCommand can attach this
            // same connection to a newly created pane's session too.
            *attached_session = Some(session_id);
            *output_rx = Some(rx);
            *conn_output_tx = Some(client_output_tx.clone());

            info!("Client {} attached to session {}", client_id, session_id);

            // Phase 4: if this workspace already has a split layout (e.g. a
            // reconnect after panes were created), attach this connection to
            // every other pane's session too — otherwise only pane-0's
            // output would ever reach this client.
            for (pane_id, other_session_id) in
                session_manager.sibling_pane_sessions(session_id).await
            {
                if other_session_id == session_id {
                    continue;
                }
                if let Err(e) = session_manager
                    .attach_client_with_user(
                        other_session_id,
                        client_id,
                        client_output_tx.clone(),
                        user_id.clone(),
                    )
                    .await
                {
                    warn!(
                        "Failed to attach client {} to sibling pane '{}' (session {}): {}",
                        client_id, pane_id, other_session_id, e
                    );
                }
            }

            // Encode scrollback for late-joiner sync with line numbers
            let scrollback_lines: Vec<monoterminal_protocol::Line> = snapshot
                .scrollback
                .iter()
                .enumerate()
                .map(|(i, line)| monoterminal_protocol::Line {
                    data: line.data.clone(),
                    line_number: i as u64,
                })
                .collect();

            // Build metadata
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            let metadata = Some(monoterminal_protocol::SessionMetadata {
                rows: snapshot.rows as u32,
                cols: snapshot.cols as u32,
                shell_type: snapshot.shell_type,
                working_dir: snapshot.working_dir.to_string_lossy().to_string(),
                created_at: now,    // TODO: Get actual created_at from session
                last_activity: now, // TODO: Get actual last_activity from session
            });

            let response = Envelope {
                sequence_number: envelope.sequence_number,
                message: Some(envelope::Message::AttachResponse(
                    monoterminal_protocol::AttachResponse {
                        session_id: session_id.to_string(),
                        metadata,
                        scrollback: scrollback_lines,
                    },
                )),
            };

            Ok(Some(response))
        }
        Some(envelope::Message::InputData(input)) => {
            debug!(
                "Processing InputData from {}: {} bytes",
                peer_addr,
                input.data.len()
            );

            // SRS §3.2.2: JWT authentication verification
            // Phase 2: Extract user_id from JWT claims for RBAC
            let user_id = if !dev_mode {
                // Production mode: verify JWT token and extract user_id
                if input.auth_token.is_empty() {
                    warn!("InputData from {} missing auth_token", peer_addr);
                    return Err(ServerError::AuthFailed(
                        "Missing authentication token".to_string(),
                    ));
                }

                let claims = verify_auth_token(auth_service, &input.auth_token)?;
                debug!(
                    "JWT verified for InputData from {}: user_id={}",
                    peer_addr, claims.sub
                );
                Some(claims.sub)
            } else {
                None
            };

            // Phase 4: Route input to pane (if pane_id specified) or focused pane
            // Backward compatibility: If pane_id is None, fall back to old behavior (attached session)
            if input.pane_id.is_some() && !input.pane_id.as_ref().unwrap().is_empty() {
                // Phase 4: Route to specified pane
                let root_session_id = attached_session.ok_or_else(|| {
                    ServerError::InvalidMessage("Not attached to session".to_string())
                })?;
                let pane_id = input.pane_id.as_deref();
                session_manager
                    .send_input_to_pane(root_session_id, pane_id, &input.data, user_id)
                    .await
                    .map_err(|e| ServerError::InvalidMessage(format!("Send input failed: {}", e)))?;
            } else {
                // Backward compatibility: Route to attached session (Phase 1-3 behavior)
                let session_id = attached_session.ok_or_else(|| {
                    ServerError::InvalidMessage("Not attached to session".to_string())
                })?;

                session_manager
                    .send_input_with_user(session_id, &input.data, user_id)
                    .await
                    .map_err(|e| ServerError::InvalidMessage(format!("Send input failed: {}", e)))?;
            }

            // No response needed for input data
            Ok(None)
        }
        Some(envelope::Message::ResizeRequest(resize)) => {
            debug!(
                "Processing ResizeRequest from {}: {}x{}",
                peer_addr, resize.rows, resize.cols
            );

            // SRS §3.2.2: JWT authentication verification
            // Phase 2: Extract user_id from JWT claims for RBAC
            let user_id = if !dev_mode {
                // Production mode: verify JWT token and extract user_id
                if resize.auth_token.is_empty() {
                    warn!("ResizeRequest from {} missing auth_token", peer_addr);
                    return Err(ServerError::AuthFailed(
                        "Missing authentication token".to_string(),
                    ));
                }

                let claims = verify_auth_token(auth_service, &resize.auth_token)?;
                debug!(
                    "JWT verified for ResizeRequest from {}: user_id={}",
                    peer_addr, claims.sub
                );
                Some(claims.sub)
            } else {
                None
            };

            // Ensure client is attached
            let root_session_id = attached_session.ok_or_else(|| {
                ServerError::InvalidMessage("Not attached to session".to_string())
            })?;

            // Phase 4: resize a specific pane (if pane_id specified), else
            // fall back to the attached/root session (Phase 1-3 behavior).
            // Each pane is its own independent PTY, so without pane-aware
            // resizing every pane but the root would desync from its actual
            // rendered size the moment the layout changes.
            if resize.pane_id.is_some() && !resize.pane_id.as_ref().unwrap().is_empty() {
                session_manager
                    .resize_pane(
                        root_session_id,
                        resize.pane_id.as_deref(),
                        resize.rows as u16,
                        resize.cols as u16,
                        user_id,
                    )
                    .await
                    .map_err(|e| ServerError::InvalidMessage(format!("Resize failed: {}", e)))?;
            } else {
                session_manager
                    .resize_session_with_user(
                        root_session_id,
                        resize.rows as u16,
                        resize.cols as u16,
                        user_id,
                    )
                    .await
                    .map_err(|e| ServerError::InvalidMessage(format!("Resize failed: {}", e)))?;
            }

            // No response needed
            Ok(None)
        }
        Some(envelope::Message::DetachRequest(_)) => {
            debug!("Processing DetachRequest from {}", peer_addr);

            // Ensure client is attached
            let session_id = attached_session.ok_or_else(|| {
                ServerError::InvalidMessage("Not attached to session".to_string())
            })?;

            // Detach from the root session and every sibling pane session
            // this connection was also attached to (Phase 4: Splits/Tabs) —
            // otherwise closing/reopening a workspace with panes would leak
            // an attachment per pane on the now-abandoned sessions.
            for (pane_id, other_session_id) in
                session_manager.sibling_pane_sessions(session_id).await
            {
                if other_session_id == session_id {
                    continue;
                }
                if let Err(e) = session_manager.detach_client(other_session_id, client_id).await {
                    warn!(
                        "Failed to detach client {} from pane '{}' (session {}): {}",
                        client_id, pane_id, other_session_id, e
                    );
                }
            }
            session_manager
                .detach_client(session_id, client_id)
                .await
                .map_err(|e| ServerError::InvalidMessage(format!("Detach failed: {}", e)))?;

            // Clear connection state
            *attached_session = None;
            *output_rx = None;
            *conn_output_tx = None;

            info!("Client {} detached from session {}", client_id, session_id);

            // No response needed
            Ok(None)
        }
        Some(envelope::Message::DashboardRequest(req)) => {
            debug!(
                "Processing DashboardRequest from {}: command={}",
                peer_addr, req.command
            );

            // Execute monomind CLI command and return JSON response
            // Commands: "status", "agents", "memory", "orgs", etc.
            let result = if req.command == "account_pairing_code" {
                execute_account_pairing_code(pairing_cache).await
            } else if req.command == "account_peer_id" {
                execute_account_peer_id(pairing_cache)
            } else {
                execute_monomind_command(&req.command, &req.params).await
            };

            let response = Envelope {
                sequence_number: envelope.sequence_number,
                message: Some(envelope::Message::DashboardResponse(
                    monoterminal_protocol::DashboardResponse {
                        json_data: result.0,
                        error: result.1 as i32,
                    },
                )),
            };

            Ok(Some(response))
        }
        Some(envelope::Message::HealthCheckRequest(req)) => {
            debug!(
                "Processing HealthCheckRequest from {}: project_dir={}",
                peer_addr, req.project_dir
            );

            // Get project directory - use session cwd if not specified
            let project_dir = if req.project_dir.is_empty() {
                attached_session
                    .and_then(|sid| session_manager.get_session_cwd(sid))
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
            } else {
                std::path::PathBuf::from(&req.project_dir)
            };

            // Run health check via monomind-bridge
            let health_status = monoterminal_monomind_bridge::run_doctor_check(&project_dir)
                .await
                .unwrap_or_else(|e| {
                    warn!("Health check failed: {}", e);
                    monoterminal_monomind_bridge::HealthStatus::not_installed()
                });

            // Convert to protobuf format
            let issues: Vec<monoterminal_protocol::HealthIssue> = health_status
                .issues
                .iter()
                .map(|issue| monoterminal_protocol::HealthIssue {
                    severity: match issue.severity {
                        monoterminal_monomind_bridge::Severity::Info => {
                            monoterminal_protocol::IssueSeverity::Info as i32
                        }
                        monoterminal_monomind_bridge::Severity::Warning => {
                            monoterminal_protocol::IssueSeverity::Warning as i32
                        }
                        monoterminal_monomind_bridge::Severity::Error => {
                            monoterminal_protocol::IssueSeverity::Error as i32
                        }
                    },
                    message: issue.message.clone(),
                    resolution: issue.resolution.clone().unwrap_or_default(),
                })
                .collect();

            let last_check_timestamp = health_status
                .last_check
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            let response = Envelope {
                sequence_number: envelope.sequence_number,
                message: Some(envelope::Message::HealthCheckResponse(
                    monoterminal_protocol::HealthCheckResponse {
                        installed: health_status.installed,
                        version: health_status.version.unwrap_or_default(),
                        control_server_reachable: health_status.control_server_reachable,
                        broker_registered: health_status.broker_registered,
                        last_check_timestamp,
                        issues,
                    },
                )),
            };

            Ok(Some(response))
        }
        Some(envelope::Message::UpgradeRequest(req)) => {
            debug!(
                "Processing UpgradeRequest from {}: project_dir={}, confirmed={}",
                peer_addr, req.project_dir, req.confirmed
            );

            // Require explicit confirmation per SRS §2.4.3
            if !req.confirmed {
                return Err(ServerError::InvalidMessage(
                    "Upgrade requires user confirmation".to_string(),
                ));
            }

            // Get project directory
            let project_dir = if req.project_dir.is_empty() {
                attached_session
                    .and_then(|sid| session_manager.get_session_cwd(sid))
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
            } else {
                std::path::PathBuf::from(&req.project_dir)
            };

            // Execute upgrade via monomind-bridge
            let upgrade_result = monoterminal_monomind_bridge::upgrade_monomind(&project_dir)
                .await
                .unwrap_or_else(|e| {
                    warn!("Upgrade failed: {}", e);
                    monoterminal_monomind_bridge::UpgradeResult {
                        success: false,
                        old_version: None,
                        new_version: None,
                        output: format!("Upgrade failed: {}", e),
                    }
                });

            let response = Envelope {
                sequence_number: envelope.sequence_number,
                message: Some(envelope::Message::UpgradeResponse(
                    monoterminal_protocol::UpgradeResponse {
                        success: upgrade_result.success,
                        old_version: upgrade_result.old_version.unwrap_or_default(),
                        new_version: upgrade_result.new_version.unwrap_or_default(),
                        output: upgrade_result.output,
                    },
                )),
            };

            Ok(Some(response))
        }
        Some(envelope::Message::DetectionRequest(req)) => {
            debug!(
                "Processing DetectionRequest from {}: project_dir={}",
                peer_addr, req.project_dir
            );

            // Get project directory
            let project_dir = if req.project_dir.is_empty() {
                attached_session
                    .and_then(|sid| session_manager.get_session_cwd(sid))
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
            } else {
                std::path::PathBuf::from(&req.project_dir)
            };

            // Detect monomind via monomind-bridge
            let detection_result = monoterminal_monomind_bridge::detect_monomind(&project_dir);

            let response = Envelope {
                sequence_number: envelope.sequence_number,
                message: Some(envelope::Message::DetectionResponse(
                    monoterminal_protocol::DetectionResponse {
                        found: detection_result.found,
                        monomind_root: detection_result
                            .monomind_root
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default(),
                        suggest_install: detection_result.suggest_install,
                        dismiss_file_exists: detection_result.dismiss_file_exists,
                        banner_text: if detection_result.suggest_install {
                            monoterminal_monomind_bridge::INSTALL_SUGGESTION_BANNER.to_string()
                        } else {
                            String::new()
                        },
                    },
                )),
            };

            Ok(Some(response))
        }
        Some(envelope::Message::SearchRequest(_req)) => {
            // Phase 4: Scrollback search (task-71, rust-engineer-protocol)
            // TODO(Week 1 Day 2-3): Implement search handler
            warn!(
                "SearchRequest not yet implemented (Phase 4 Week 1 Day 2-3)"
            );
            Err(ServerError::InvalidMessage(
                "Search feature not yet implemented".to_string(),
            ))
        }
        Some(envelope::Message::SplitPaneCommand(cmd)) => {
            // Phase 4 Week 2: Splits/Tabs handler (ADR-018, task-73)
            debug!(
                "Processing SplitPaneCommand from {}: pane_id={}, direction={:?}",
                peer_addr, cmd.pane_id, cmd.direction
            );

            // Extract user_id from JWT for RBAC
            let user_id = if !dev_mode {
                if cmd.pane_id.is_empty() {
                    return Err(ServerError::InvalidMessage(
                        "SplitPaneCommand missing pane_id".to_string(),
                    ));
                }
                // Note: Auth token would be in a separate field in a real implementation
                // For now, user_id is extracted from session context
                None
            } else {
                None
            };

            let root_session_id = attached_session.ok_or_else(|| {
                ServerError::InvalidMessage("Not attached to session".to_string())
            })?;

            // Call SessionManager to handle split (dimensions derived from existing pane's session)
            let result = session_manager
                .handle_split_pane(
                    root_session_id,
                    &cmd.pane_id,
                    crate::layout::SplitDirection::from(cmd.direction()),
                    Some(cmd.new_session_shell.clone()),
                    user_id.clone(),
                )
                .await
                .map_err(|e| ServerError::InvalidMessage(format!("Split pane failed: {}", e)))?;

            // Attach this connection to the new pane's session too — without
            // this, the new pane would exist in the layout but never
            // actually stream any terminal output to this client.
            if let Some(tx) = conn_output_tx.as_ref() {
                if let Err(e) = session_manager
                    .attach_client_with_user(result.new_session_id, client_id, tx.clone(), user_id)
                    .await
                {
                    warn!(
                        "Failed to attach client {} to new pane '{}' (session {}): {}",
                        client_id, result.new_pane_id, result.new_session_id, e
                    );
                }
            }

            // Return LayoutUpdate to client
            let response = Envelope {
                sequence_number: envelope.sequence_number,
                message: Some(envelope::Message::LayoutUpdate(result.layout_update)),
            };

            Ok(Some(response))
        }
        Some(envelope::Message::ClosePaneCommand(cmd)) => {
            // Phase 4 Week 2: Splits/Tabs handler (ADR-018, task-73)
            debug!(
                "Processing ClosePaneCommand from {}: pane_id={}",
                peer_addr, cmd.pane_id
            );

            // Extract user_id from JWT for RBAC
            let user_id = if !dev_mode {
                if cmd.pane_id.is_empty() {
                    return Err(ServerError::InvalidMessage(
                        "ClosePaneCommand missing pane_id".to_string(),
                    ));
                }
                None
            } else {
                None
            };

            let root_session_id = attached_session.ok_or_else(|| {
                ServerError::InvalidMessage("Not attached to session".to_string())
            })?;

            // Call SessionManager to handle close
            let layout_update = session_manager
                .handle_close_pane(root_session_id, &cmd.pane_id, user_id)
                .await
                .map_err(|e| ServerError::InvalidMessage(format!("Close pane failed: {}", e)))?;

            // Return LayoutUpdate to client
            let response = Envelope {
                sequence_number: envelope.sequence_number,
                message: Some(envelope::Message::LayoutUpdate(layout_update)),
            };

            Ok(Some(response))
        }
        Some(envelope::Message::FocusPaneCommand(cmd)) => {
            // Phase 4 Week 2: Splits/Tabs handler (ADR-018, task-73)
            debug!(
                "Processing FocusPaneCommand from {}: pane_id={}",
                peer_addr, cmd.pane_id
            );

            if cmd.pane_id.is_empty() {
                return Err(ServerError::InvalidMessage(
                    "FocusPaneCommand missing pane_id".to_string(),
                ));
            }

            let root_session_id = attached_session.ok_or_else(|| {
                ServerError::InvalidMessage("Not attached to session".to_string())
            })?;

            // Call SessionManager to handle focus
            let layout_update = session_manager
                .handle_focus_pane(root_session_id, &cmd.pane_id)
                .await
                .map_err(|e| ServerError::InvalidMessage(format!("Focus pane failed: {}", e)))?;

            // Return LayoutUpdate to client
            let response = Envelope {
                sequence_number: envelope.sequence_number,
                message: Some(envelope::Message::LayoutUpdate(layout_update)),
            };

            Ok(Some(response))
        }
        Some(envelope::Message::ClipboardGetRequest(req)) => {
            // Phase 4 Week 2: Bidirectional Clipboard (ADR-020, task-74)
            debug!(
                "Processing ClipboardGetRequest from {}: request_id={}",
                peer_addr, req.request_id
            );

            // Extract session_id from attached_session
            let session_id = attached_session
                .ok_or_else(|| {
                    ServerError::InvalidMessage("Client not attached to session".to_string())
                })?;

            // user_id: Session-level auth (verified at AttachRequest)
            // TODO: Extract user_id from session state once RBAC is fully implemented
            let user_id = "session-user".to_string();

            // Call ClipboardManager
            match clipboard_manager
                .handle_clipboard_get(session_id, user_id, peer_addr)
                .await
            {
                Ok(content) => {
                    let response = Envelope {
                        sequence_number: envelope.sequence_number,
                        message: Some(envelope::Message::ClipboardGetResponse(
                            monoterminal_protocol::ClipboardGetResponse {
                                request_id: req.request_id,
                                content,
                                mime_type: "text/plain".to_string(),
                                authorized: true,
                                error: String::new(),
                            },
                        )),
                    };
                    Ok(Some(response))
                }
                Err(e) => {
                    // Return error response to client
                    let error_msg = match e {
                        crate::clipboard::ClipboardError::RateLimitExceeded { retry_after } => {
                            format!("Rate limit exceeded. Retry after {} seconds", retry_after)
                        }
                        crate::clipboard::ClipboardError::AuthorizationRequired => {
                            "Authorization required for clipboard access".to_string()
                        }
                        _ => format!("Clipboard error: {}", e),
                    };

                    let response = Envelope {
                        sequence_number: envelope.sequence_number,
                        message: Some(envelope::Message::ClipboardGetResponse(
                            monoterminal_protocol::ClipboardGetResponse {
                                request_id: req.request_id,
                                content: String::new(),
                                mime_type: String::new(),
                                authorized: false,
                                error: error_msg,
                            },
                        )),
                    };
                    Ok(Some(response))
                }
            }
        }
        Some(envelope::Message::ClipboardSetRequest(req)) => {
            // Phase 4 Week 2: Bidirectional Clipboard (ADR-020, task-74)
            debug!(
                "Processing ClipboardSetRequest from {}: {} bytes",
                peer_addr,
                req.content.len()
            );

            // Extract session_id from attached_session
            let session_id = attached_session
                .ok_or_else(|| {
                    ServerError::InvalidMessage("Client not attached to session".to_string())
                })?;

            // user_id: Session-level auth (verified at AttachRequest)
            // TODO: Extract user_id from session state once RBAC is fully implemented
            let user_id = "session-user".to_string();

            // Call ClipboardManager
            clipboard_manager
                .handle_clipboard_set(session_id, req.content, user_id, peer_addr)
                .await
                .map_err(|e| {
                    let error_msg = match e {
                        crate::clipboard::ClipboardError::RateLimitExceeded { retry_after } => {
                            format!("Rate limit exceeded. Retry after {} seconds", retry_after)
                        }
                        crate::clipboard::ClipboardError::SizeLimitExceeded => {
                            "Clipboard size exceeds 1MB limit".to_string()
                        }
                        _ => format!("Clipboard error: {}", e),
                    };
                    ServerError::InvalidMessage(error_msg)
                })?;

            // No response for clipboard set (fire-and-forget)
            Ok(None)
        }
        Some(envelope::Message::ClipboardGetResponse(_resp)) => {
            // Server → Client message (unexpected from client)
            warn!(
                "Received unexpected ClipboardGetResponse from {}",
                peer_addr
            );
            Err(ServerError::InvalidMessage(
                "Client sent server→client message type".to_string(),
            ))
        }
        Some(envelope::Message::AttachResponse(_))
        | Some(envelope::Message::OutputData(_))
        | Some(envelope::Message::ErrorResponse(_))
        | Some(envelope::Message::DashboardResponse(_))
        | Some(envelope::Message::HealthCheckResponse(_))
        | Some(envelope::Message::UpgradeResponse(_))
        | Some(envelope::Message::DetectionResponse(_))
        | Some(envelope::Message::MonitoringData(_))
        | Some(envelope::Message::WebrtcOffer(_))
        | Some(envelope::Message::WebrtcAnswer(_))
        | Some(envelope::Message::IceCandidate(_))
        | Some(envelope::Message::SearchResponse(_))
        | Some(envelope::Message::LayoutUpdate(_))
        | Some(envelope::Message::ClipboardOsc52(_)) => {
            warn!(
                "Received unexpected server->client or P2P message from {}",
                peer_addr
            );
            Err(ServerError::InvalidMessage(
                "Client sent server/P2P message type".to_string(),
            ))
        }
        None => {
            warn!("Received envelope with no message from {}", peer_addr);
            Err(ServerError::InvalidMessage("Empty envelope".to_string()))
        }
    }
}

/// Execute monomind CLI command and return JSON response
///
/// Executes commands like "status", "agents", "memory", "orgs" via monomind CLI
/// and returns the JSON output for the dashboard.
///
/// # Arguments
///
/// * `command` - Command name ("status", "agents", etc.)
/// * `params` - Optional parameters map
///
/// # Returns
///
/// * `(String, ErrorCode)` - (JSON response, error code)
async fn execute_monomind_command(
    command: &str,
    params: &std::collections::HashMap<String, String>,
) -> (String, ErrorCode) {
    use std::process::Command;

    debug!("Executing monomind command: {}", command);

    // Build command arguments
    let mut args = vec![
        "monomind@latest".to_string(),
        command.to_string(),
        "--json".to_string(),
    ];

    // Add params as arguments
    for (key, value) in params {
        args.push(format!("--{}", key));
        args.push(value.clone());
    }

    // Execute command
    let result =
        tokio::task::spawn_blocking(move || Command::new("npx").args(&args).output()).await;

    match result {
        Ok(Ok(output)) if output.status.success() => {
            let json = String::from_utf8_lossy(&output.stdout).to_string();
            (json, ErrorCode::Unknown)
        }
        Ok(Ok(output)) => {
            let error_msg = String::from_utf8_lossy(&output.stderr).to_string();
            warn!("Monomind command failed: {}", error_msg);
            (
                serde_json::json!({
                    "error": error_msg,
                    "exitCode": output.status.code(),
                })
                .to_string(),
                ErrorCode::ServerError,
            )
        }
        Ok(Err(e)) => {
            error!("Failed to execute monomind command: {}", e);
            (
                serde_json::json!({
                    "error": format!("Command execution failed: {}", e),
                })
                .to_string(),
                ErrorCode::ServerError,
            )
        }
        Err(e) => {
            error!("Failed to spawn monomind command: {}", e);
            (
                serde_json::json!({
                    "error": format!("Task join failed: {}", e),
                })
                .to_string(),
                ErrorCode::ServerError,
            )
        }
    }
}

/// Return this daemon's peer_id (Ed25519 pubkey hex) — a pure local read, so
/// this is synchronous unlike `execute_account_pairing_code`. Lets a browser
/// that's already directly, authentically connected to this daemon check
/// whether it's already linked to the caller's account before asking for a
/// pairing code.
///
/// # Returns
///
/// * `(String, ErrorCode)` - (JSON response, error code), same convention as
///   `execute_account_pairing_code`.
fn execute_account_peer_id(pairing_cache: Option<&Arc<PairingCodeCache>>) -> (String, ErrorCode) {
    let Some(cache) = pairing_cache else {
        return (
            serde_json::json!({
                "error": "P2P is not enabled on this daemon (no --relay-url configured)"
            })
            .to_string(),
            ErrorCode::ServerError,
        );
    };

    (
        serde_json::json!({ "peer_id": cache.peer_id() }).to_string(),
        ErrorCode::Unknown,
    )
}

/// Fetch (or return the cached) SaaS device-pairing code for this daemon.
///
/// # Returns
///
/// * `(String, ErrorCode)` - (JSON response, error code). Success uses
///   `ErrorCode::Unknown` as the "no error" sentinel, matching
///   `execute_monomind_command`'s convention.
async fn execute_account_pairing_code(
    pairing_cache: Option<&Arc<PairingCodeCache>>,
) -> (String, ErrorCode) {
    let Some(cache) = pairing_cache else {
        return (
            serde_json::json!({
                "error": "P2P is not enabled on this daemon (no --relay-url configured)"
            })
            .to_string(),
            ErrorCode::ServerError,
        );
    };

    match cache.get_or_refresh().await {
        Ok(code) => (
            serde_json::json!({
                "code": code.code,
                "expires_at": code.expires_at,
            })
            .to_string(),
            ErrorCode::Unknown,
        ),
        Err(e) => {
            warn!("Failed to fetch pairing code: {}", e);
            (
                serde_json::json!({ "error": e.to_string() }).to_string(),
                ErrorCode::ServerError,
            )
        }
    }
}

/// Create error envelope from ServerError
fn create_error_envelope(sequence_number: u64, error: ServerError) -> Envelope {
    let (code, message) = match error {
        ServerError::SessionNotFound(msg) => (
            monoterminal_protocol::ErrorCode::SessionNotFound as i32,
            msg,
        ),
        ServerError::AuthFailed(msg) => (monoterminal_protocol::ErrorCode::AuthFailed as i32, msg),
        ServerError::PermissionDenied => (
            monoterminal_protocol::ErrorCode::PermissionDenied as i32,
            "Permission denied".to_string(),
        ),
        ServerError::RateLimitExceeded => (
            monoterminal_protocol::ErrorCode::RateLimitExceeded as i32,
            "Rate limit exceeded".to_string(),
        ),
        _ => (
            monoterminal_protocol::ErrorCode::Unknown as i32,
            format!("{}", error),
        ),
    };

    Envelope {
        sequence_number,
        message: Some(envelope::Message::ErrorResponse(
            monoterminal_protocol::ErrorResponse { code, message },
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_error_envelope() {
        let error = ServerError::SessionNotFound("session-123".to_string());
        let envelope = create_error_envelope(42, error);

        assert_eq!(envelope.sequence_number, 42);

        match envelope.message {
            Some(envelope::Message::ErrorResponse(err)) => {
                assert_eq!(
                    err.code,
                    monoterminal_protocol::ErrorCode::SessionNotFound as i32
                );
                assert_eq!(err.message, "session-123");
            }
            _ => panic!("Expected ErrorResponse"),
        }
    }
}
