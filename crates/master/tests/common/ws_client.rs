// WebSocket test client simulator
// As specified in test-strategy-phase1.md §6.2

use anyhow::Result;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async, connect_async_tls_with_config, tungstenite::Message, Connector, MaybeTlsStream,
    WebSocketStream,
};

/// Test WebSocket client for integration tests
pub struct TestWsClient {
    stream: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    #[allow(dead_code)]
    url: String,
    #[allow(dead_code)]
    accept_invalid_certs: bool,
}

impl TestWsClient {
    /// Create a new test client (not yet connected)
    #[allow(dead_code)]
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            stream: None,
            url: url.into(),
            accept_invalid_certs: false,
        }
    }

    /// Create a new test client that accepts self-signed/invalid certificates
    #[allow(dead_code)]
    pub fn new_accept_invalid_certs(url: impl Into<String>) -> Self {
        Self {
            stream: None,
            url: url.into(),
            accept_invalid_certs: true,
        }
    }

    /// Connect to the WebSocket server
    #[allow(dead_code)]
    pub async fn connect(&mut self) -> Result<()> {
        let (stream, _response) = if self.accept_invalid_certs {
            // Use rustls (not native-tls) to accept the server's self-signed
            // dev cert: the server is TLS 1.3-only (see server/tls.rs), and
            // native-tls's macOS backend (Security.framework/Secure
            // Transport) caps out at TLS 1.2, so it can never complete this
            // handshake on macOS ("bad protocol version"). rustls is a pure
            // Rust TLS stack with no such OS-backend cap, so it works
            // identically on every platform.
            let tls_config = rustls022::ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(std::sync::Arc::new(NoCertVerification))
                .with_no_client_auth();

            connect_async_tls_with_config(
                &self.url,
                None,
                false,
                Some(Connector::Rustls(std::sync::Arc::new(tls_config))),
            )
            .await?
        } else {
            connect_async(&self.url).await?
        };

        self.stream = Some(stream);
        Ok(())
    }

    /// Send a binary message
    pub async fn send_binary(&mut self, data: Vec<u8>) -> Result<()> {
        if let Some(ref mut stream) = self.stream {
            use futures_util::SinkExt;
            stream.send(Message::Binary(data)).await?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Not connected"))
        }
    }

    /// Receive a message
    pub async fn recv(&mut self) -> Result<Message> {
        if let Some(ref mut stream) = self.stream {
            use futures_util::StreamExt;
            let msg = stream
                .next()
                .await
                .ok_or_else(|| anyhow::anyhow!("Stream closed"))??;
            Ok(msg)
        } else {
            Err(anyhow::anyhow!("Not connected"))
        }
    }

    /// Close the connection
    #[allow(dead_code)]
    #[allow(unused_imports)]
    pub async fn close(&mut self) -> Result<()> {
        if let Some(mut stream) = self.stream.take() {
            use futures_util::SinkExt;
            stream.close(None).await?;
        }
        Ok(())
    }

    /// Check if connected
    #[allow(dead_code)]
    pub fn is_connected(&self) -> bool {
        self.stream.is_some()
    }

    /// Send AttachRequest and wait for AttachResponse
    #[allow(dead_code)]
    pub async fn attach(
        &mut self,
        session_id: &str,
        jwt_bearer: &str,
        rows: u32,
        cols: u32,
    ) -> Result<monoterminal_protocol::AttachResponse> {
        use prost::Message as ProstMessage;

        let attach_req = monoterminal_protocol::AttachRequest {
            session_id: session_id.to_string(),
            auth_token: jwt_bearer.to_owned(),
            rows,
            cols,
            last_seen_sequence: 0,
            session_name: String::new(),
        };

        let envelope = monoterminal_protocol::Envelope {
            sequence_number: 1,
            message: Some(monoterminal_protocol::envelope::Message::AttachRequest(
                attach_req,
            )),
        };

        let mut buf = Vec::with_capacity(envelope.encoded_len());
        envelope.encode(&mut buf)?;
        self.send_binary(buf).await?;

        let response_msg = self.recv().await?;
        match response_msg {
            Message::Binary(data) => {
                let response_envelope = monoterminal_protocol::Envelope::decode(&data[..])?;
                match response_envelope.message {
                    Some(monoterminal_protocol::envelope::Message::AttachResponse(resp)) => {
                        Ok(resp)
                    }
                    Some(monoterminal_protocol::envelope::Message::ErrorResponse(err)) => Err(
                        anyhow::anyhow!("Attach failed: {} (code: {})", err.message, err.code),
                    ),
                    _ => Err(anyhow::anyhow!("Unexpected response type")),
                }
            }
            _ => Err(anyhow::anyhow!("Expected binary response")),
        }
    }

    /// Send input data to attached session
    #[allow(dead_code)]
    pub async fn send_input(&mut self, data: &[u8], jwt_bearer: &str) -> Result<()> {
        use prost::Message as ProstMessage;

        let input_data = monoterminal_protocol::InputData {
            data: data.to_vec(),
            pane_id: None,
            auth_token: jwt_bearer.to_owned(),
        };

        let envelope = monoterminal_protocol::Envelope {
            sequence_number: 2,
            message: Some(monoterminal_protocol::envelope::Message::InputData(
                input_data,
            )),
        };

        let mut buf = Vec::with_capacity(envelope.encoded_len());
        envelope.encode(&mut buf)?;
        self.send_binary(buf).await?;

        Ok(())
    }

    /// Send resize request
    #[allow(dead_code)]
    pub async fn resize(&mut self, rows: u32, cols: u32, jwt_bearer: &str) -> Result<()> {
        use prost::Message as ProstMessage;

        let resize_req = monoterminal_protocol::ResizeRequest {
            rows,
            cols,
            pane_id: None,
            auth_token: jwt_bearer.to_owned(),
        };

        let envelope = monoterminal_protocol::Envelope {
            sequence_number: 3,
            message: Some(monoterminal_protocol::envelope::Message::ResizeRequest(
                resize_req,
            )),
        };

        let mut buf = Vec::with_capacity(envelope.encoded_len());
        envelope.encode(&mut buf)?;
        self.send_binary(buf).await?;

        Ok(())
    }

    /// Send detach request
    #[allow(dead_code)]
    pub async fn detach(&mut self, session_id: &str) -> Result<()> {
        use prost::Message as ProstMessage;

        let detach_req = monoterminal_protocol::DetachRequest {
            session_id: session_id.to_string(),
        };

        let envelope = monoterminal_protocol::Envelope {
            sequence_number: 4,
            message: Some(monoterminal_protocol::envelope::Message::DetachRequest(
                detach_req,
            )),
        };

        let mut buf = Vec::with_capacity(envelope.encoded_len());
        envelope.encode(&mut buf)?;
        self.send_binary(buf).await?;

        Ok(())
    }
}

/// Accepts any server certificate — mirrors native-tls's
/// `danger_accept_invalid_certs(true)` for connecting to the self-signed dev
/// TLS cert. Test-only: never use for a real connection.
#[derive(Debug)]
struct NoCertVerification;

impl rustls022::client::danger::ServerCertVerifier for NoCertVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls022::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls022::pki_types::CertificateDer<'_>],
        _server_name: &rustls022::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls022::pki_types::UnixTime,
    ) -> Result<rustls022::client::danger::ServerCertVerified, rustls022::Error> {
        Ok(rustls022::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls022::pki_types::CertificateDer<'_>,
        _dss: &rustls022::DigitallySignedStruct,
    ) -> Result<rustls022::client::danger::HandshakeSignatureValid, rustls022::Error> {
        Ok(rustls022::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls022::pki_types::CertificateDer<'_>,
        _dss: &rustls022::DigitallySignedStruct,
    ) -> Result<rustls022::client::danger::HandshakeSignatureValid, rustls022::Error> {
        Ok(rustls022::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls022::SignatureScheme> {
        rustls022::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_client_creation() {
        use super::*;
        let client = TestWsClient::new("ws://127.0.0.1:8080");
        assert!(!client.is_connected());
    }
}
