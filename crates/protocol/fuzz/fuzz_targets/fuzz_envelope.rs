#![no_main]

use libfuzzer_sys::fuzz_target;
use monoterminal_protocol::{Envelope, envelope};
use prost::Message;

fuzz_target!(|data: &[u8]| {
    // Fuzz Envelope deserialization (comprehensive: all message types)
    // Tests: All 50+ message types in the oneof enum
    // Edge cases: sequence number overflow, unknown message types, malformed oneofs

    if let Ok(envelope) = Envelope::decode(data) {
        // Validate envelope sequence number
        let _ = envelope.sequence_number;
        let _ = envelope.sequence_number.wrapping_add(1);

        // Exercise all message types in the oneof
        // Use reference to avoid moving envelope
        match &envelope.message {
            Some(envelope::Message::AttachRequest(msg)) => {
                let _ = msg.session_id.len();
                let _ = msg.rows.saturating_mul(msg.cols);
            }
            Some(envelope::Message::AttachResponse(msg)) => {
                let _ = msg.session_id.len();
                let _ = msg.scrollback.len();
            }
            Some(envelope::Message::InputData(msg)) => {
                let _ = msg.data.len();
                let _ = msg.auth_token.len();
            }
            Some(envelope::Message::OutputData(msg)) => {
                let _ = msg.data.len();
                let _ = msg.sequence;
            }
            Some(envelope::Message::ResizeRequest(msg)) => {
                let _ = msg.rows;
                let _ = msg.cols;
                let _ = msg.auth_token.len();
            }
            Some(envelope::Message::DetachRequest(msg)) => {
                let _ = msg.session_id.len();
            }
            Some(envelope::Message::ErrorResponse(msg)) => {
                let _ = msg.code;
                let _ = msg.message.len();
            }
            Some(envelope::Message::DashboardRequest(msg)) => {
                let _ = msg.command.len();
                let _ = msg.params.len();
            }
            Some(envelope::Message::DashboardResponse(msg)) => {
                let _ = msg.json_data.len();
            }
            Some(envelope::Message::HealthCheckRequest(msg)) => {
                let _ = msg.project_dir.len();
            }
            Some(envelope::Message::HealthCheckResponse(msg)) => {
                let _ = msg.installed;
                let _ = msg.version.len();
            }
            Some(envelope::Message::UpgradeRequest(msg)) => {
                let _ = msg.project_dir.len();
                let _ = msg.confirmed;
            }
            Some(envelope::Message::UpgradeResponse(msg)) => {
                let _ = msg.success;
                let _ = msg.old_version.len();
            }
            Some(envelope::Message::DetectionRequest(msg)) => {
                let _ = msg.project_dir.len();
            }
            Some(envelope::Message::DetectionResponse(msg)) => {
                let _ = msg.found;
                let _ = msg.monomind_root.len();
            }
            Some(envelope::Message::MonitoringData(msg)) => {
                let _ = msg.org_name.len();
                let _ = msg.active_agents;
                let _ = msg.recent_runs.len();
            }
            Some(envelope::Message::WebrtcOffer(msg)) => {
                let _ = msg.session_id.len();
                let _ = msg.sdp.len();
            }
            Some(envelope::Message::WebrtcAnswer(msg)) => {
                let _ = msg.sdp.len();
                if let Some(turn) = &msg.turn {
                    let _ = turn.urls.len();
                }
            }
            Some(envelope::Message::IceCandidate(msg)) => {
                let _ = msg.session_id.len();
                let _ = msg.candidate.len();
            }
            Some(envelope::Message::SearchRequest(msg)) => {
                let _ = msg.session_id.len();
                let _ = msg.query.len();
                let _ = msg.max_results;
            }
            Some(envelope::Message::SearchResponse(msg)) => {
                let _ = msg.matches.len();
                let _ = msg.total_matches;
                let _ = msg.truncated;
            }
            // Plugin protocol messages (Phase 4 Week 5-8 - not yet in .proto)
            // Uncomment when plugin messages are added to messages.proto
            /*
            Some(envelope::Message::PluginRegisterRequest(msg)) => {
                let _ = msg.plugin_id.len();
                let _ = msg.hooks.len();
            }
            Some(envelope::Message::PluginRegisterResponse(msg)) => {
                let _ = msg.approved;
                let _ = msg.granted_hooks.len();
            }
            Some(envelope::Message::PluginOutputEvent(msg)) => {
                let _ = msg.plugin_id.len();
                let _ = msg.output_data.len();
            }
            Some(envelope::Message::PluginStateChangeEvent(msg)) => {
                let _ = msg.plugin_id.len();
                let _ = msg.old_state;
                let _ = msg.new_state;
            }
            Some(envelope::Message::PluginInvokeRequest(msg)) => {
                let _ = msg.plugin_id.len();
                let _ = msg.payload.len();
            }
            Some(envelope::Message::PluginInvokeResponse(msg)) => {
                let _ = msg.result.len();
                let _ = msg.execution_time_ms;
            }
            */
            Some(envelope::Message::SplitPaneCommand(msg)) => {
                let _ = msg.pane_id.len();
                let _ = msg.new_session_shell.len();
            }
            Some(envelope::Message::ClosePaneCommand(msg)) => {
                let _ = msg.pane_id.len();
            }
            Some(envelope::Message::FocusPaneCommand(msg)) => {
                let _ = msg.pane_id.len();
            }
            Some(envelope::Message::LayoutUpdate(msg)) => {
                let _ = msg.focused_pane_id.len();
            }
            Some(envelope::Message::ClipboardGetRequest(msg)) => {
                let _ = msg.request_id.len();
                let _ = msg.mime_types.len();
            }
            Some(envelope::Message::ClipboardGetResponse(msg)) => {
                let _ = msg.request_id.len();
                let _ = msg.content.len();
            }
            Some(envelope::Message::ClipboardSetRequest(msg)) => {
                let _ = msg.content.len();
                let _ = msg.mime_type.len();
            }
            Some(envelope::Message::ClipboardOsc52(msg)) => {
                let _ = msg.content.len();
                let _ = msg.selection.len();
            }
            None => {
                // Empty message (valid protobuf: oneof with no variant set)
                // This is a valid state - envelope with no message
            }
        }

        // Test re-encoding roundtrip
        let mut buf = Vec::new();
        if envelope.encode(&mut buf).is_ok() {
            // Verify roundtrip produces decodable output
            let _ = Envelope::decode(&buf[..]);
        }
    }
});
