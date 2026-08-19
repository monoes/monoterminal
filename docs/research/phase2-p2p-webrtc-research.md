# Phase 2 P2P WebRTC Research

**Date:** 2026-08-15  
**Status:** Design Phase (Implementation blocked on Criteria #1 until ~2026-08-17 22:00)  
**Owner:** networking-engineer  
**References:** ADR-003, SRS §7.2

---

## Table of Contents

1. [WebRTC Architecture Patterns](#webrtc-architecture-patterns)
2. [rust-webrtc Integration](#rust-webrtc-integration)
3. [Signaling Server Design](#signaling-server-design)
4. [NAT Traversal Strategy](#nat-traversal-strategy)
5. [coturn TURN Relay Setup](#coturn-turn-relay-setup)
6. [Discovery Service Architecture](#discovery-service-architecture)
7. [Connection Limits & Quotas](#connection-limits--quotas)
8. [Mobile Browser Considerations](#mobile-browser-considerations)

---

## 1. WebRTC Architecture Patterns

### 1.1 Connection Establishment Flow

**Standard WebRTC Flow:**

```
Client                    Signaling Server              Master
  |                              |                         |
  |--- Discover Master --------->|                         |
  |<-- Master Info -------------|                         |
  |                              |                         |
  |--- Create Offer (SDP) ------>|--- Forward Offer ------>|
  |                              |                         |
  |                              |<-- Create Answer (SDP)--|
  |<-- Forward Answer -----------|                         |
  |                              |                         |
  |--- ICE Candidates ---------->|--- Forward ICE -------->|
  |<-- ICE Candidates -----------|<-- Forward ICE ---------|
  |                              |                         |
  |<=============P2P DataChannel Connection===============>|
```

**Key Phases:**
1. **Discovery:** Client finds master's address (mDNS/directory service)
2. **Signaling:** SDP offer/answer exchange via WebSocket
3. **ICE:** Candidate gathering and exchange (STUN/TURN)
4. **Connection:** P2P DataChannel established (DTLS-SRTP encrypted)

### 1.2 Connection Models

**Option A: Client-Initiated (Recommended)**
- Client discovers master, initiates WebRTC offer
- Master responds with answer
- **Pros:** Natural for client-server model, master is passive
- **Cons:** Requires master to be discoverable

**Option B: Master-Initiated**
- Master broadcasts availability, clients respond
- Master initiates offer to each client
- **Pros:** More control over connection process
- **Cons:** More complex, less standard

**Decision: Use Option A (client-initiated)** — aligns with standard WebRTC patterns and simplifies master implementation.

### 1.3 ICE Candidate Exchange

**Trickle ICE (Recommended):**
- Send candidates as they're discovered (incremental)
- Faster connection establishment (~2-3s vs ~5-8s)
- More complex state management

**Batch ICE:**
- Wait for all candidates, send in one message
- Simpler implementation
- Slower connection (~5-8s)

**Decision: Use Trickle ICE** — speed is critical for UX, especially on mobile.

---

## 2. rust-webrtc Integration

### 2.1 Crate Selection

**Primary Crate: `webrtc`**
- Repository: https://github.com/webrtc-rs/webrtc
- Version: 0.9.x (stable)
- Features: DataChannel, DTLS, SRTP, ICE, STUN, TURN client

**Dependencies:**
```toml
[dependencies]
webrtc = "0.9"
tokio = { version = "1.35", features = ["full"] }
```

### 2.2 RTCPeerConnection Setup

**Rust Master:**

```rust
use webrtc::api::APIBuilder;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::ice_transport::ice_server::RTCIceServer;

async fn create_peer_connection() -> Result<RTCPeerConnection, Error> {
    let config = RTCConfiguration {
        ice_servers: vec![
            // Public STUN servers
            RTCIceServer {
                urls: vec!["stun:stun.l.google.com:19302".to_string()],
                ..Default::default()
            },
            // Self-hosted TURN relay
            RTCIceServer {
                urls: vec![
                    "turn:turn.monoterminal.dev:3478".to_string(),
                    "turns:turn.monoterminal.dev:5349".to_string(),
                ],
                username: "monoterminal".to_string(),
                credential: "TURN_SECRET".to_string(), // From config
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let api = APIBuilder::new().build();
    api.new_peer_connection(config).await
}
```

### 2.3 DataChannel Setup

**Create DataChannel:**

```rust
async fn setup_data_channel(pc: &RTCPeerConnection) -> Result<Arc<RTCDataChannel>, Error> {
    let data_channel = pc.create_data_channel("monoterminal", None).await?;
    
    // Handle incoming messages
    data_channel.on_message(Box::new(|msg: DataChannelMessage| {
        Box::pin(async move {
            // Parse Protobuf message
            let proto_msg = parse_protobuf(&msg.data)?;
            handle_client_message(proto_msg).await?;
            Ok(())
        })
    }));
    
    // Handle channel state changes
    data_channel.on_open(Box::new(|| {
        Box::pin(async move {
            println!("DataChannel opened");
            Ok(())
        })
    }));
    
    Ok(data_channel)
}
```

### 2.4 SDP Offer/Answer Handling

**Create Offer (Client):**

```rust
async fn create_offer(pc: &RTCPeerConnection) -> Result<String, Error> {
    let offer = pc.create_offer(None).await?;
    pc.set_local_description(offer.clone()).await?;
    Ok(offer.sdp)
}
```

**Handle Answer (Client):**

```rust
async fn handle_answer(pc: &RTCPeerConnection, sdp: String) -> Result<(), Error> {
    let answer = RTCSessionDescription::answer(sdp)?;
    pc.set_remote_description(answer).await?;
    Ok(())
}
```

**Create Answer (Master):**

```rust
async fn create_answer(pc: &RTCPeerConnection, offer_sdp: String) -> Result<String, Error> {
    let offer = RTCSessionDescription::offer(offer_sdp)?;
    pc.set_remote_description(offer).await?;
    
    let answer = pc.create_answer(None).await?;
    pc.set_local_description(answer.clone()).await?;
    
    Ok(answer.sdp)
}
```

### 2.5 ICE Candidate Handling

**Gather and Send ICE Candidates (Both Sides):**

```rust
async fn setup_ice_handlers(pc: &RTCPeerConnection, signaling: SignalingChannel) -> Result<(), Error> {
    pc.on_ice_candidate(Box::new(move |candidate: Option<RTCIceCandidate>| {
        let signaling = signaling.clone();
        Box::pin(async move {
            if let Some(c) = candidate {
                // Send candidate to peer via signaling channel
                signaling.send_ice_candidate(c).await?;
            }
            Ok(())
        })
    }));
    
    Ok(())
}
```

**Handle Incoming ICE Candidates:**

```rust
async fn add_ice_candidate(pc: &RTCPeerConnection, candidate: RTCIceCandidate) -> Result<(), Error> {
    pc.add_ice_candidate(candidate).await?;
    Ok(())
}
```

---

## 3. Signaling Server Design

### 3.1 Signaling Server Options

**Option A: Embedded in Master Daemon (Recommended for Phase 2)**

**Pros:**
- No extra deployment
- Simpler for single-user/small-team use
- Direct access to session state

**Cons:**
- Master must be reachable for signaling (NAT issue if master is behind NAT)
- Doesn't scale to many masters

**Option B: Separate Signaling Service**

**Pros:**
- Scales better (one service, many masters)
- Master doesn't need to be publicly reachable for signaling
- Can run on same VPS as directory service

**Cons:**
- Extra deployment complexity
- Another service to maintain

**Decision for Phase 2: Option A (embedded)** — simpler for MVP. Can move to Option B in Phase 4 for enterprise scaling.

### 3.2 Signaling Protocol

**WebSocket-based signaling over existing connection:**

```protobuf
// Signaling messages (extend existing protocol schema)

message WebRTCOffer {
  string session_id = 1;
  string sdp = 2;
  string peer_id = 3;  // Client's Ed25519 public key
}

message WebRTCAnswer {
  string session_id = 1;
  string sdp = 2;
}

message ICECandidate {
  string session_id = 1;
  string candidate = 2;
  string sdp_mid = 3;
  uint32 sdp_mline_index = 4;
}

message WebRTCError {
  string session_id = 1;
  string error_message = 2;
}
```

**Flow:**
1. Client connects via WebSocket (existing Phase 1 connection)
2. Client sends `WebRTCOffer` over WebSocket
3. Master sends `WebRTCAnswer` over WebSocket
4. Both sides exchange `ICECandidate` messages over WebSocket
5. Once P2P DataChannel is established, client can optionally close WebSocket (or keep as fallback)

### 3.3 Signaling Security

**Authentication:**
- Reuse existing Ed25519/JWT auth (Phase 1)
- Only authenticated clients can initiate WebRTC negotiation
- Master validates `peer_id` matches authenticated client's public key

**Rate Limiting:**
- Max 5 WebRTC offers per client per minute (prevent DOS)
- Max 100 ICE candidates per offer (prevent spam)

---

## 4. NAT Traversal Strategy

### 4.1 Three-Tier Fallback Chain

**From ADR-003:**

```
┌─────────────────────────────────────────────────────┐
│ Tier 1: STUN Direct Connection (timeout: 10s)      │
│   - Public STUN servers (Google)                    │
│   - Success rate: 60-95% (depends on NAT type)      │
│   - Cost: Free                                      │
└─────────────────────────────────────────────────────┘
                    │
                    │ FAILS
                    ▼
┌─────────────────────────────────────────────────────┐
│ Tier 2: TURN Relay (timeout: 5s)                   │
│   - Self-hosted coturn relay                        │
│   - Success rate: 98-99%                            │
│   - Cost: $5-15/month VPS + bandwidth               │
└─────────────────────────────────────────────────────┘
                    │
                    │ FAILS (coturn unavailable)
                    ▼
┌─────────────────────────────────────────────────────┐
│ Tier 3: HTTPS Relay (no timeout)                   │
│   - Master acts as WebSocket relay                  │
│   - Success rate: 100% (always works)               │
│   - Cost: Free (uses existing WebSocket connection) │
└─────────────────────────────────────────────────────┘
```

### 4.2 NAT Traversal Success Rates (Literature)

**From ADR-003 and libp2p research:**

| Network Type | STUN Direct | TURN Fallback | HTTPS Relay |
|--------------|-------------|---------------|-------------|
| WiFi (home) | 85-95% | 98-99% | 100% |
| Cellular (4G/5G) | 60-75% | 98-99% | 100% |
| Corporate VPN | 40-55% | 98-99% | 100% |
| Symmetric NAT | 10-20% | 98-99% | 100% |

**CRITICAL NOTE (from SRS §7.2):**
> Real success rates must be measured in Phase 2 against actual MONOTERMINAL traffic — don't assume literature figures.

**Phase 2 Acceptance:** 65-80% NAT traversal success (measured directly).

### 4.3 NAT Type Detection

**Strategy:**
- Use STUN binding request to detect NAT type
- Categorize as: Full Cone, Restricted Cone, Port-Restricted Cone, Symmetric
- Log NAT type for telemetry

**Implementation:**

```rust
async fn detect_nat_type(stun_server: &str) -> Result<NATType, Error> {
    // RFC 3489 NAT type detection algorithm
    // Requires multiple STUN queries from different ports
    // Returns: FullCone, RestrictedCone, PortRestrictedCone, Symmetric
    unimplemented!("NAT detection via STUN")
}
```

### 4.4 Connection Timeout Strategy

**Tier 1 (STUN):** 10s timeout
- Fast enough for good UX
- Long enough for ICE gathering on slow networks

**Tier 2 (TURN):** 5s timeout
- TURN should be faster (dedicated relay)
- If TURN fails in 5s, likely unavailable

**Tier 3 (HTTPS):** No timeout
- Fallback always succeeds (existing WebSocket connection)

---

## 5. coturn TURN Relay Setup

### 5.1 Server Requirements

**Minimal VPS Specs:**
- **CPU:** 1 vCPU (2 vCPU for >100 concurrent connections)
- **RAM:** 512 MB (1 GB recommended)
- **Bandwidth:** 100 GB/month minimum
- **Cost:** $5-15/month (DigitalOcean, Linode, Vultr)

**Recommended VPS:**
- DigitalOcean Droplet: $6/month (1 vCPU, 1 GB RAM, 25 GB SSD, 1000 GB transfer)
- Linode Nanode: $5/month (1 vCPU, 1 GB RAM, 25 GB SSD, 1 TB transfer)

### 5.2 coturn Installation

**Ubuntu 22.04 Setup:**

```bash
# Install coturn
sudo apt update
sudo apt install coturn -y

# Enable coturn service
sudo systemctl enable coturn
```

### 5.3 coturn Configuration

**File:** `/etc/turnserver.conf`

```ini
# Listening ports
listening-port=3478
tls-listening-port=5349

# External IP (VPS public IP)
external-ip=<VPS_PUBLIC_IP>

# Realm (domain)
realm=turn.monoterminal.dev
server-name=turn.monoterminal.dev

# Authentication
lt-cred-mech
user=monoterminal:TURN_SECRET

# SSL/TLS certificates (Let's Encrypt)
cert=/etc/letsencrypt/live/turn.monoterminal.dev/fullchain.pem
pkey=/etc/letsencrypt/live/turn.monoterminal.dev/privkey.pem

# Quota and rate limiting
max-bps=1000000  # 1 Mbps per allocation
total-quota=100  # Max 100 allocations
bps-capacity=0   # No global bandwidth limit

# Logging
log-file=/var/log/coturn/turnserver.log
verbose

# Security
no-cli
no-loopback-peers
no-multicast-peers
stale-nonce=600
```

### 5.4 Firewall Configuration

**Open ports:**

```bash
# STUN/TURN ports
sudo ufw allow 3478/tcp
sudo ufw allow 3478/udp
sudo ufw allow 5349/tcp
sudo ufw allow 5349/udp

# Relay port range (ephemeral ports for TURN)
sudo ufw allow 49152:65535/tcp
sudo ufw allow 49152:65535/udp
```

### 5.5 TLS Certificate Setup

**Let's Encrypt (certbot):**

```bash
# Install certbot
sudo apt install certbot -y

# Obtain certificate
sudo certbot certonly --standalone -d turn.monoterminal.dev

# Auto-renewal (cron)
sudo crontab -e
# Add: 0 3 * * * certbot renew --quiet && systemctl restart coturn
```

### 5.6 Health Monitoring

**Check coturn status:**

```bash
# Service status
sudo systemctl status coturn

# Check logs
sudo tail -f /var/log/coturn/turnserver.log

# Test TURN relay
turnutils_uclient -v -u monoterminal -w TURN_SECRET <VPS_PUBLIC_IP>
```

**Prometheus Metrics (Future):**
- Expose coturn metrics via `prometheus-turnserver-exporter`
- Monitor allocation count, bandwidth usage, error rates

---

## 6. Discovery Service Architecture

### 6.1 Hybrid Discovery Model

**From ADR-003:**

| Method | Scope | Latency | Reliability | Use Case |
|--------|-------|---------|-------------|----------|
| **mDNS/Bonjour** | LAN only | 1-5s | HIGH | Same WiFi network |
| **Directory Service** | Internet | <100ms | MEDIUM | Different networks |
| **Kademlia DHT** | Internet | 2-10s | HIGH | Future (Phase 4+) |

### 6.2 mDNS/Bonjour Discovery (LAN)

**Master Advertisement:**

```rust
use mdns_sd::{ServiceDaemon, ServiceInfo};

async fn advertise_master() -> Result<(), Error> {
    let mdns = ServiceDaemon::new()?;
    
    let service_type = "_monoterminal._tcp.local.";
    let instance_name = "MonoTerminal Master";
    let port = 8080;
    
    let service = ServiceInfo::new(
        service_type,
        instance_name,
        &format!("monoterminal-{}.local.", hostname()),
        (),
        port,
        Some(vec![
            ("peer_id", peer_id_base64()),
            ("version", "1.0.0"),
        ])
    )?;
    
    mdns.register(service)?;
    Ok(())
}
```

**Client Discovery:**

```rust
use mdns_sd::{ServiceDaemon, ServiceEvent};

async fn discover_masters_mdns() -> Result<Vec<MasterInfo>, Error> {
    let mdns = ServiceDaemon::new()?;
    let service_type = "_monoterminal._tcp.local.";
    
    let receiver = mdns.browse(service_type)?;
    let mut masters = vec![];
    
    // Listen for 2 seconds
    let timeout = tokio::time::sleep(Duration::from_secs(2));
    tokio::pin!(timeout);
    
    loop {
        tokio::select! {
            event = receiver.recv() => {
                match event? {
                    ServiceEvent::ServiceResolved(info) => {
                        masters.push(MasterInfo {
                            address: info.get_addresses().iter().next().unwrap().clone(),
                            port: info.get_port(),
                            peer_id: info.get_property("peer_id").unwrap().val_str(),
                        });
                    }
                    _ => {}
                }
            }
            _ = &mut timeout => break,
        }
    }
    
    Ok(masters)
}
```

### 6.3 Directory Service (Internet)

**Architecture:**

```
┌─────────────┐          ┌─────────────────┐          ┌─────────────┐
│   Master    │          │   Directory     │          │   Client    │
│             │          │    Service      │          │             │
│  (daemon)   │          │  (HTTP REST)    │          │  (browser)  │
└─────────────┘          └─────────────────┘          └─────────────┘
       │                          │                           │
       │  POST /api/v1/register   │                           │
       │─────────────────────────>│                           │
       │  (heartbeat every 30s)   │                           │
       │                          │  GET /api/v1/discover     │
       │                          │<──────────────────────────│
       │                          │  [{master1}, {master2}]   │
       │                          │───────────────────────────>│
```

**API Endpoints:**

```yaml
POST /api/v1/register
  Request:
    peer_id: string (Ed25519 public key, base64)
    address: string (IP or domain)
    port: uint16
    version: string
    metadata: object (optional)
  Response:
    registered_at: timestamp
    ttl: uint32 (seconds, e.g. 60)

GET /api/v1/discover
  Query Parameters:
    peer_id: string (optional, filter by specific master)
    version: string (optional, filter by version)
    limit: uint32 (default: 100)
  Response:
    masters: array[MasterInfo]
      peer_id: string
      address: string
      port: uint16
      version: string
      last_seen: timestamp

DELETE /api/v1/unregister
  Request:
    peer_id: string
  Response:
    success: bool
```

**Master Registration (Heartbeat):**

```rust
async fn register_with_directory(client: &DirectoryClient) -> Result<(), Error> {
    let interval = Duration::from_secs(30);
    
    loop {
        client.post("/api/v1/register")
            .json(&json!({
                "peer_id": peer_id_base64(),
                "address": public_address(),
                "port": 8080,
                "version": "1.0.0",
            }))
            .send()
            .await?;
        
        tokio::time::sleep(interval).await;
    }
}
```

**Client Discovery:**

```rust
async fn discover_masters_directory(client: &DirectoryClient) -> Result<Vec<MasterInfo>, Error> {
    let resp = client.get("/api/v1/discover")
        .send()
        .await?;
    
    let body: DiscoverResponse = resp.json().await?;
    Ok(body.masters)
}
```

### 6.4 Result Merge & Deduplication

**Flow:**

```rust
async fn discover_all_masters() -> Result<Vec<MasterInfo>, Error> {
    // Run in parallel
    let (mdns_results, directory_results) = tokio::join!(
        discover_masters_mdns(),
        discover_masters_directory(directory_client),
    );
    
    // Merge results
    let mut all_masters = vec![];
    all_masters.extend(mdns_results?);
    all_masters.extend(directory_results?);
    
    // Deduplicate by peer_id
    let mut seen = HashSet::new();
    all_masters.retain(|m| seen.insert(m.peer_id.clone()));
    
    // Sort: LAN (mDNS) first, then Internet (directory)
    all_masters.sort_by_key(|m| m.source != DiscoverySource::MDNS);
    
    Ok(all_masters)
}

enum DiscoverySource {
    MDNS,
    Directory,
}
```

---

## 7. Connection Limits & Quotas

### 7.1 Requirements (from SRS §2.3.4)

**Global Limits:**
- **Max total connections:** 1000
- **Max clients per session:** 50
- **New connections rate limit:** 100 connections/minute

**Platform-specific quota enforcement:**
- **Windows:** Job Objects (memory, CPU)
- **Linux:** cgroups v2 (Phase 3)
- **macOS:** launchd limits (Phase 3)

### 7.2 Connection Tracking

```rust
struct ConnectionManager {
    // Global connection count
    total_connections: AtomicUsize,
    max_total: usize,  // 1000
    
    // Per-session connection count
    session_connections: HashMap<SessionId, usize>,
    max_per_session: usize,  // 50
    
    // Rate limiting (sliding window)
    connection_timestamps: VecDeque<Instant>,
    rate_limit_window: Duration,  // 1 minute
    rate_limit_max: usize,  // 100
}

impl ConnectionManager {
    fn can_accept_connection(&mut self, session_id: &SessionId) -> Result<(), ConnectionError> {
        // Check global limit
        if self.total_connections.load(Ordering::Relaxed) >= self.max_total {
            return Err(ConnectionError::GlobalLimitReached);
        }
        
        // Check per-session limit
        if let Some(&count) = self.session_connections.get(session_id) {
            if count >= self.max_per_session {
                return Err(ConnectionError::SessionLimitReached);
            }
        }
        
        // Check rate limit (sliding window)
        let now = Instant::now();
        let window_start = now - self.rate_limit_window;
        
        // Remove old timestamps
        while let Some(&ts) = self.connection_timestamps.front() {
            if ts < window_start {
                self.connection_timestamps.pop_front();
            } else {
                break;
            }
        }
        
        // Check rate
        if self.connection_timestamps.len() >= self.rate_limit_max {
            return Err(ConnectionError::RateLimitExceeded);
        }
        
        // Allow connection
        self.connection_timestamps.push_back(now);
        self.total_connections.fetch_add(1, Ordering::Relaxed);
        *self.session_connections.entry(session_id.clone()).or_insert(0) += 1;
        
        Ok(())
    }
}
```

### 7.3 Windows Job Objects (Phase 1)

**Resource Quotas:**

```rust
#[cfg(target_os = "windows")]
fn set_job_object_limits() -> Result<(), Error> {
    use winapi::um::jobapi2::*;
    use winapi::um::winnt::*;
    
    unsafe {
        let job = CreateJobObjectW(null_mut(), null_mut());
        
        // Memory limit: 4 GB
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.ProcessMemoryLimit = 4 * 1024 * 1024 * 1024; // 4 GB
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_PROCESS_MEMORY;
        
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *mut _,
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        
        // Assign current process to job
        AssignProcessToJobObject(job, GetCurrentProcess());
    }
    
    Ok(())
}
```

---

## 8. Mobile Browser Considerations

### 8.1 iOS Safari Background Behavior

**From SRS §2.2, §9.3:**

> **iOS Safari backgrounding trade-off:** When Safari moves to background, WebSocket connections are suspended after ~30s. User must return to the app to resume. WebRTC behaves similarly.

**Challenge:**
- iOS Safari suspends all network activity after ~30s in background
- WebRTC DataChannel is also suspended
- User must return to app to reconnect

**Acceptance Criteria (SRS §7.2):**
> Reconnect-after-background works reliably on iOS Safari — <10s target

### 8.2 Reconnection Strategy

**Approach:**

1. **Detect background/foreground transitions:**
   - Use Page Visibility API: `document.addEventListener('visibilitychange', ...)`
   
2. **On background:** Accept that connection will suspend (no workaround)

3. **On foreground:**
   - Detect connection loss (DataChannel closed or WebSocket disconnected)
   - Automatically reconnect:
     - Try WebRTC first (if STUN/TURN still available)
     - Fall back to WebSocket if WebRTC fails
   - Request late-joiner scrollback (last 1000 lines)
   - Resume session

**Implementation:**

```javascript
// Web client (React + TypeScript)

document.addEventListener('visibilitychange', () => {
  if (document.visibilityState === 'visible') {
    // App returned to foreground
    if (!isConnected()) {
      // Reconnect
      reconnect({ requestScrollback: true, scrollbackLines: 1000 });
    }
  }
});

async function reconnect(options) {
  const startTime = performance.now();
  
  try {
    // Try WebRTC first
    await connectWebRTC();
  } catch (err) {
    // Fall back to WebSocket
    await connectWebSocket();
  }
  
  const elapsed = performance.now() - startTime;
  console.log(`Reconnected in ${elapsed}ms`);
  
  // Telemetry: track reconnection time
  reportReconnectionTime(elapsed);
}
```

### 8.3 Android Chrome Background Behavior

**Better than iOS:**
- Android Chrome allows background tabs to maintain WebSocket connections (with restrictions)
- WebRTC may stay alive longer (~5 minutes vs ~30s on iOS)
- Less aggressive suspension

**Strategy:** Same reconnection flow, but likely faster reconnect on Android.

### 8.4 Reconnection Telemetry

**Track:**
- Reconnection time (ms)
- Reconnection success rate (WebRTC vs WebSocket fallback)
- Platform (iOS Safari vs Android Chrome)

**Goal:** Validate <10s reconnection target (SRS §7.2 acceptance).

---

## Next Steps

1. **Architecture Design Session** with principal-architect (Monday 2026-08-17)
   - Finalize signaling server design (embedded vs separate)
   - Finalize discovery service API
   - Finalize session attach protocol

2. **Protocol Schema Design** with rust-engineer-protocol (Monday 2026-08-17)
   - Multi-client attach messages
   - Presence indicators
   - WebRTC signaling messages

3. **Implementation** (starts Tuesday 2026-08-18, after Criteria #1 unblocks)
   - rust-webrtc integration
   - Signaling server
   - coturn TURN relay setup
   - Discovery service (directory)
   - Connection limits/quotas
   - Reconnection logic (web client)

4. **Testing & Validation** (Phase 2 acceptance)
   - NAT traversal success rate measurement (65-80% target)
   - 100 concurrent sessions test
   - iOS Safari reconnection validation (<10s target)
   - Test coverage (75% target)

---

**Status:** Research complete, ready for architecture design session.
