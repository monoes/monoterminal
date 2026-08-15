// WebSocket message handler with Protocol Buffer integration
// Implements task-3: Protocol Runtime Integration
// Implements task-2: Monomind bridge integration (health/upgrade/detection/dashboard)
// Implements task-8: JWT authentication integration

use std::net::SocketAddr;
use std::sync::Arc;
use std::path::PathBuf;
use futures_util::{SinkExt, StreamExt};
use prost::Message;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_rustls::server::TlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::protocol::Message as WsMessage;
use tracing::{debug, info, warn, error};
use uuid::Uuid;

use monoterminal_protocol::{Envelope, envelope, ErrorCode};
use crate::auth::{AuthService, Claims};
use crate::session::manager::SessionManager;
use crate::session::{SessionId, ClientId};
use super::error::{ServerError, Result};

/// Handle WebSocket connection with bidirectional streaming
/// Client → Server: AttachRequest, InputData, ResizeRequest, DetachRequest
/// Server → Client: AttachResponse, OutputData (continuous), ErrorResponse
pub async fn handle_websocket(
    ws_stream: WebSocketStream<TlsStream<TcpStream>>,
    peer_addr: SocketAddr,
    session_manager: Arc<SessionManager>,
) -> Result<()> {
    let (mut ws_write, mut ws_read) = ws_stream.split();
    let mut sequence_number: u64 = 0;
    let client_id = Uuid::new_v4();

    info!("WebSocket handler started for {} (client_id: {})", peer_addr, client_id);

    // Connection state
    let mut attached_session: Option<SessionId> = None;
    let mut output_rx: Option<mpsc::Receiver<Vec<u8>>> = None;

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
                                    client_id,
                                    &mut attached_session,
                                    &mut output_rx,
                                    peer_addr,
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
            error!("Failed to detach client {} from session {}: {}", client_id, session_id, e);
        } else {
            info!("Client {} detached from session {} on disconnect", client_id, session_id);
        }
    }

    info!("WebSocket handler stopped for {}", peer_addr);
    Ok(())
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
fn verify_auth_token(
    auth_service: &dyn AuthService,
    token: &str,
) -> Result<Claims> {
    auth_service
        .verify_access(token)
        .map_err(|e| ServerError::AuthFailed(format!("JWT verification failed: {}", e)))
}

/// Process a Protocol Buffer message
async fn process_message(
    envelope: Envelope,
    session_manager: &SessionManager,
    client_id: ClientId,
    attached_session: &mut Option<SessionId>,
    output_rx: &mut Option<mpsc::Receiver<Vec<u8>>>,
    peer_addr: SocketAddr,
) -> Result<Option<Envelope>> {
    match envelope.message {
        Some(envelope::Message::AttachRequest(req)) => {
            debug!("Processing AttachRequest from {}: session_id={}", peer_addr, req.session_id);

            // Parse session_id from string
            let session_id = Uuid::parse_str(&req.session_id)
                .map_err(|e| ServerError::InvalidMessage(format!("Invalid session_id UUID: {}", e)))?;

            // Create output channel for this client
            let (output_tx, rx) = mpsc::channel(256); // 256 messages ≈ 1MB buffer per SRS §3.1.4

            // Attach client to session
            let snapshot = session_manager
                .attach_client(session_id, client_id, output_tx)
                .await
                .map_err(|e| match e {
                    crate::session::SessionError::NotFound(_) =>
                        ServerError::SessionNotFound(req.session_id.clone()),
                    _ => ServerError::InvalidMessage(format!("Attach failed: {}", e)),
                })?;

            // Store connection state
            *attached_session = Some(session_id);
            *output_rx = Some(rx);

            info!("Client {} attached to session {}", client_id, session_id);

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
                created_at: now,  // TODO: Get actual created_at from session
                last_activity: now,  // TODO: Get actual last_activity from session
            });

            let response = Envelope {
                sequence_number: envelope.sequence_number,
                message: Some(envelope::Message::AttachResponse(
                    monoterminal_protocol::AttachResponse {
                        session_id: req.session_id,
                        metadata,
                        scrollback: scrollback_lines,
                    }
                )),
            };

            Ok(Some(response))
        }
        Some(envelope::Message::InputData(input)) => {
            debug!("Processing InputData from {}: {} bytes", peer_addr, input.data.len());

            // Ensure client is attached
            let session_id = attached_session
                .ok_or_else(|| ServerError::InvalidMessage("Not attached to session".to_string()))?;

            // Forward input to session PTY
            session_manager
                .send_input(session_id, &input.data)
                .await
                .map_err(|e| ServerError::InvalidMessage(format!("Send input failed: {}", e)))?;

            // No response needed for input data
            Ok(None)
        }
        Some(envelope::Message::ResizeRequest(resize)) => {
            debug!("Processing ResizeRequest from {}: {}x{}", peer_addr, resize.rows, resize.cols);

            // Ensure client is attached
            let session_id = attached_session
                .ok_or_else(|| ServerError::InvalidMessage("Not attached to session".to_string()))?;

            // Resize session PTY
            session_manager
                .resize_session(session_id, resize.rows as u16, resize.cols as u16)
                .await
                .map_err(|e| ServerError::InvalidMessage(format!("Resize failed: {}", e)))?;

            // No response needed
            Ok(None)
        }
        Some(envelope::Message::DetachRequest(_)) => {
            debug!("Processing DetachRequest from {}", peer_addr);

            // Ensure client is attached
            let session_id = attached_session
                .ok_or_else(|| ServerError::InvalidMessage("Not attached to session".to_string()))?;

            // Detach from session
            session_manager
                .detach_client(session_id, client_id)
                .await
                .map_err(|e| ServerError::InvalidMessage(format!("Detach failed: {}", e)))?;

            // Clear connection state
            *attached_session = None;
            *output_rx = None;

            info!("Client {} detached from session {}", client_id, session_id);

            // No response needed
            Ok(None)
        }
        Some(envelope::Message::DashboardRequest(req)) => {
            debug!("Processing DashboardRequest from {}: command={}", peer_addr, req.command);

            // Execute monomind CLI command and return JSON response
            // Commands: "status", "agents", "memory", "orgs", etc.
            let result = execute_monomind_command(&req.command, &req.params).await;

            let response = Envelope {
                sequence_number: envelope.sequence_number,
                message: Some(envelope::Message::DashboardResponse(
                    monoterminal_protocol::DashboardResponse {
                        json_data: result.0,
                        error: result.1 as i32,
                    }
                )),
            };

            Ok(Some(response))
        }
        Some(envelope::Message::HealthCheckRequest(req)) => {
            debug!("Processing HealthCheckRequest from {}: project_dir={}", peer_addr, req.project_dir);

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
            let issues: Vec<monoterminal_protocol::HealthIssue> = health_status.issues
                .iter()
                .map(|issue| monoterminal_protocol::HealthIssue {
                    severity: match issue.severity {
                        monoterminal_monomind_bridge::Severity::Info => monoterminal_protocol::IssueSeverity::Info as i32,
                        monoterminal_monomind_bridge::Severity::Warning => monoterminal_protocol::IssueSeverity::Warning as i32,
                        monoterminal_monomind_bridge::Severity::Error => monoterminal_protocol::IssueSeverity::Error as i32,
                    },
                    message: issue.message.clone(),
                    resolution: issue.resolution.clone().unwrap_or_default(),
                })
                .collect();

            let last_check_timestamp = health_status.last_check
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
                    }
                )),
            };

            Ok(Some(response))
        }
        Some(envelope::Message::UpgradeRequest(req)) => {
            debug!("Processing UpgradeRequest from {}: project_dir={}, confirmed={}",
                peer_addr, req.project_dir, req.confirmed);

            // Require explicit confirmation per SRS §2.4.3
            if !req.confirmed {
                return Err(ServerError::InvalidMessage(
                    "Upgrade requires user confirmation".to_string()
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
                    }
                )),
            };

            Ok(Some(response))
        }
        Some(envelope::Message::DetectionRequest(req)) => {
            debug!("Processing DetectionRequest from {}: project_dir={}", peer_addr, req.project_dir);

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
                        monomind_root: detection_result.monomind_root
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default(),
                        suggest_install: detection_result.suggest_install,
                        dismiss_file_exists: detection_result.dismiss_file_exists,
                        banner_text: if detection_result.suggest_install {
                            monoterminal_monomind_bridge::INSTALL_SUGGESTION_BANNER.to_string()
                        } else {
                            String::new()
                        },
                    }
                )),
            };

            Ok(Some(response))
        }
        Some(envelope::Message::AttachResponse(_)) |
        Some(envelope::Message::OutputData(_)) |
        Some(envelope::Message::ErrorResponse(_)) |
        Some(envelope::Message::DashboardResponse(_)) |
        Some(envelope::Message::HealthCheckResponse(_)) |
        Some(envelope::Message::UpgradeResponse(_)) |
        Some(envelope::Message::DetectionResponse(_)) |
        Some(envelope::Message::MonitoringData(_)) => {
            warn!("Received unexpected server->client message from {}", peer_addr);
            Err(ServerError::InvalidMessage("Client sent server message type".to_string()))
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
    let mut args = vec!["monomind@latest".to_string(), command.to_string(), "--json".to_string()];

    // Add params as arguments
    for (key, value) in params {
        args.push(format!("--{}", key));
        args.push(value.clone());
    }

    // Execute command
    let result = tokio::task::spawn_blocking(move || {
        Command::new("npx")
            .args(&args)
            .output()
    })
    .await;

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
                }).to_string(),
                ErrorCode::ServerError
            )
        }
        Ok(Err(e)) => {
            error!("Failed to execute monomind command: {}", e);
            (
                serde_json::json!({
                    "error": format!("Command execution failed: {}", e),
                }).to_string(),
                ErrorCode::ServerError
            )
        }
        Err(e) => {
            error!("Failed to spawn monomind command: {}", e);
            (
                serde_json::json!({
                    "error": format!("Task join failed: {}", e),
                }).to_string(),
                ErrorCode::ServerError
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
        ServerError::AuthFailed(msg) => (
            monoterminal_protocol::ErrorCode::AuthFailed as i32,
            msg,
        ),
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
            monoterminal_protocol::ErrorResponse { code, message }
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
                assert_eq!(err.code, monoterminal_protocol::ErrorCode::SessionNotFound as i32);
                assert_eq!(err.message, "session-123");
            }
            _ => panic!("Expected ErrorResponse"),
        }
    }
}
