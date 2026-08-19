# ADR-005: Daemon Lifecycle - Windows Service Implementation

**Status:** ✅ APPROVED (eng-director, 2026-08-15 21:10)  
**Date:** 2026-08-15  
**Deciders:** eng-director, principal-architect, rust-backend-lead  
**SRS Reference:** §7.1 (Windows Service)  
**Phase:** Phase 1 (Windows master daemon)

---

## Context

MONOTERMINAL master daemon (SRS §7.1) runs as a Windows Service on Windows 10 1809+ (ConPTY). The service must:

- **Start on boot** (no socket activation - Windows has no launchd/systemd equivalent)
- **Run continuously** (accept WebSocket connections, manage PTY sessions)
- **Integrate HealthScheduler** (daily monomind doctor checks per SRS §2.4.3)
- **Handle graceful shutdown** (flush sessions, close connections, cleanup resources)
- **Support dual-mode operation** (Service mode for production, Console mode for development)

**Trigger for this ADR:**
- Architecture review (2026-08-15) identified HealthScheduler daemon integration pattern as design decision needed before task-7 (Windows Service) implementation
- rust-backend-lead provided comprehensive feedback on service lifecycle patterns

---

## Decision

Implement **dual-mode daemon** with the following patterns:

1. **HealthScheduler Integration:** Spawn on Service Start (tokio background task with caching)
2. **Resource Paths:** Dual-mode (Service=%ProgramData%, Console=%LOCALAPPDATA%)
3. **Logging:** File-based (Phase 1), Windows Event Log deferred (Phase 2)
4. **Graceful Shutdown:** 10-second timeout with force-kill fallback
5. **Service Control:** Built-in CLI commands (--install/--uninstall/--start/--stop)

---

## Topic 1: HealthScheduler Integration

### Decision: Spawn on Service Start (OPTION A) ✅

**Pattern:** tokio background task with 5-minute health check caching

```rust
// Windows Service OnStart handler (or main() in Console mode)
use std::sync::Arc;
use tokio::sync::RwLock;
use monoterminal_monomind_bridge::{HealthScheduler, HealthStatus};

pub struct ServiceState {
    health_handle: tokio::task::JoinHandle<()>,
    cached_health: Arc<RwLock<Option<HealthStatus>>>,
}

impl ServiceState {
    pub async fn new(project_dir: PathBuf) -> Self {
        let cached_health = Arc::new(RwLock::new(None));
        let cache_clone = cached_health.clone();
        
        // Spawn HealthScheduler as background task
        let health_handle = tokio::spawn(async move {
            let scheduler = HealthScheduler::new(); // 24h interval
            
            scheduler.start(&project_dir, |health| {
                let cache = cache_clone.clone();
                async move {
                    // Cache health status for 5 minutes
                    *cache.write().await = Some(health.clone());
                    
                    // Broadcast to connected clients
                    broadcast_health_to_clients(health).await;
                }
            }).await.expect("HealthScheduler failed");
        });
        
        Self { health_handle, cached_health }
    }
    
    // Get cached health status (avoids npx spam)
    pub async fn get_health(&self) -> Option<HealthStatus> {
        self.cached_health.read().await.clone()
    }
    
    // Force refresh (on explicit user request)
    pub async fn refresh_health(&self, project_dir: &Path) -> Result<HealthStatus> {
        let health = run_doctor_check(project_dir).await?;
        *self.cached_health.write().await = Some(health.clone());
        Ok(health)
    }
}

// Windows Service OnStop handler (or Ctrl+C in Console mode)
impl Drop for ServiceState {
    fn drop(&mut self) {
        self.health_handle.abort(); // Graceful shutdown
    }
}
```

**Health check execution schedule:**
- ✅ **24h scheduled check:** HealthScheduler timer (automatic)
- ✅ **On user request:** Dashboard "refresh" button (explicit)
- ✅ **Session creation:** Per-session detection only (not full health check)
- ❌ **NOT on every API call:** Use cached status (5-minute cache)

**Rationale:**
- ✅ Matches SRS §2.4.3 daily scheduler requirement
- ✅ Clean tokio integration (no Windows Task Scheduler external dependency)
- ✅ Graceful shutdown via abort() on service stop
- ✅ Prevents npx spam (5-minute cache, per rust-backend-lead)

**Alternative Rejected:** Manual tick (call run_doctor_check() on-demand only)
- ❌ Violates SRS §2.4.3 "daily background health check scheduler"
- ❌ No automatic monitoring (users must remember to check)

---

## Topic 2: Certificate & Log Path Resolution

### Decision: Dual-Mode (Service vs Console) ✅

**Service Mode** (production, runs as Windows Service):
- Requires admin installation
- Uses `%ProgramData%\MONOTERMINAL\` (e.g., `C:\ProgramData\MONOTERMINAL\`)
- Service account has Write permission (installer pre-creates directories with ACLs)

**Console Mode** (development, runs as normal process):
- No admin required
- Uses `%LOCALAPPDATA%\monoterminal\` (e.g., `C:\Users\Alice\AppData\Local\monoterminal\`)
- User's own directories (no ACL setup needed)

**Implementation:**

```rust
use std::env;
use std::path::PathBuf;

pub enum RunMode {
    Service,  // Running as Windows Service
    Console,  // Running in console mode (development)
}

impl RunMode {
    /// Detect current run mode
    pub fn detect() -> Self {
        // Check if running as Windows Service (no console attached)
        if unsafe { winapi::um::wincon::GetConsoleWindow().is_null() } {
            RunMode::Service
        } else {
            RunMode::Console
        }
    }
    
    /// Get base data directory for this run mode
    pub fn data_dir(&self) -> PathBuf {
        match self {
            RunMode::Service => {
                // %ProgramData%\MONOTERMINAL\
                PathBuf::from(env::var("ProgramData").expect("ProgramData not set"))
                    .join("MONOTERMINAL")
            }
            RunMode::Console => {
                // %LOCALAPPDATA%\monoterminal\
                PathBuf::from(env::var("LOCALAPPDATA").expect("LOCALAPPDATA not set"))
                    .join("monoterminal")
            }
        }
    }
}

/// Get certificate directory with priority order
pub fn get_cert_dir(mode: RunMode) -> PathBuf {
    // Priority 1: Environment variable override
    if let Ok(cert_dir) = env::var("MONOTERMINAL_CERT_DIR") {
        return PathBuf::from(cert_dir);
    }
    
    // Priority 2: Config file (if exists)
    let config_path = mode.data_dir().join("config.toml");
    if let Ok(config) = load_config(&config_path) {
        if let Some(cert_dir) = config.cert_dir {
            return PathBuf::from(cert_dir);
        }
    }
    
    // Priority 3: Mode-based default
    mode.data_dir().join("certs")
}

/// Get log directory (same resolution order)
pub fn get_log_dir(mode: RunMode) -> PathBuf {
    if let Ok(log_dir) = env::var("MONOTERMINAL_LOG_DIR") {
        return PathBuf::from(log_dir);
    }
    
    mode.data_dir().join("logs")
}
```

**Path Resolution Priority:**
1. **Environment variable** `MONOTERMINAL_CERT_DIR` / `MONOTERMINAL_LOG_DIR` (highest)
2. **Config file** `<data_dir>/config.toml` (if exists)
3. **Mode-based default** `<data_dir>/certs` or `<data_dir>/logs`

**Rationale:**
- ✅ Development workflow doesn't require admin rights (per rust-backend-lead)
- ✅ Production uses standard Windows Service paths (%ProgramData%)
- ✅ Environment variable override for CI/CD or custom deployments
- ✅ Matches Windows best practices (LocalAppData for user apps, ProgramData for services)

**Installer Requirements:**
- ✅ Service mode: Installer MUST pre-create directories with ACLs (service account needs Write)
- ✅ Console mode: Directories created on-demand by application (user owns %LOCALAPPDATA%)

---

## Topic 3: Logging Strategy

### Decision: File Logging (Phase 1), Event Log Deferred (Phase 2) ✅

**Phase 1 Implementation:**

```rust
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, EnvFilter, prelude::*};

pub fn init_logging(mode: RunMode) -> Result<()> {
    let log_dir = get_log_dir(mode);
    
    // Create log directory if missing
    std::fs::create_dir_all(&log_dir)?;
    
    // Rolling file appender (10MB max, 5 files kept)
    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("monoterminal")
        .filename_suffix("log")
        .max_log_files(5)
        .build(log_dir)?;
    
    // Logging configuration
    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive("monoterminal=info".parse()?))
        .with(
            fmt::layer()
                .with_writer(file_appender)
                .with_ansi(false) // Plain text for file logs
                .with_target(true)
                .with_thread_ids(true)
        );
    
    tracing::subscriber::set_global_default(subscriber)?;
    Ok(())
}
```

**Log Paths:**
- Service mode: `%ProgramData%\MONOTERMINAL\logs\monoterminal-YYYY-MM-DD.log`
- Console mode: `%LOCALAPPDATA%\monoterminal\logs\monoterminal-YYYY-MM-DD.log`

**Rotation Policy:**
- Daily rotation (new file per day)
- 5 files kept (older files auto-deleted)
- Size limit: 10MB per file (fallback if daily rotation insufficient)

**Phase 2+ (Future):**
- ✅ Add Windows Event Log subscriber (`tracing-windows-event-log` crate)
- ✅ Event Log requires admin to create event source (installer step)
- ✅ Can add without changing log call sites (tracing subscriber swap)

**Rationale:**
- ✅ File logging sufficient for Phase 1 MVP debugging
- ✅ Event Log requires admin permissions to create event source (complicates installer)
- ✅ Dual-mode paths match certificate/data directory pattern
- ✅ Can upgrade to Event Log in Phase 2 transparently (per rust-backend-lead)

---

## Topic 4: Graceful Shutdown Pattern

### Decision: 10-Second Timeout with Force-Kill Fallback ✅

**Shutdown Sequence:**

```rust
use tokio::time::{timeout, Duration};
use futures::future::join_all;

pub async fn graceful_shutdown(
    listener: TcpListener,
    sessions: Vec<Arc<Session>>,
    health_handle: JoinHandle<()>,
    rendering_handle: JoinHandle<()>,
) -> Result<()> {
    tracing::info!("Received shutdown signal, starting graceful shutdown");
    
    // Step 1: Stop accepting new connections
    drop(listener);
    tracing::info!("Stopped accepting new connections");
    
    // Step 2: Send graceful shutdown signal to all sessions (Ctrl+C to PTY)
    let shutdown_futures: Vec<_> = sessions.iter()
        .map(|session| session.send_shutdown_signal())
        .collect();
    
    // Step 3: Wait up to 10 seconds for graceful shutdown
    match timeout(Duration::from_secs(10), join_all(shutdown_futures)).await {
        Ok(_) => {
            tracing::info!("All sessions shut down gracefully");
        }
        Err(_) => {
            tracing::warn!(
                "Timeout waiting for sessions to shut down, force killing {} sessions",
                sessions.len()
            );
            
            // Force kill remaining sessions
            for session in &sessions {
                if let Err(e) = session.force_kill().await {
                    tracing::error!("Failed to force kill session: {}", e);
                }
            }
        }
    }
    
    // Step 4: Abort background tasks
    health_handle.abort();
    rendering_handle.abort();
    tracing::info!("Aborted background tasks");
    
    // Step 5: Flush logs
    tracing::info!("Flushing logs and exiting");
    // tracing automatically flushes on Drop, but explicit flush here for clarity
    
    Ok(())
}
```

**Session Shutdown Implementation:**

```rust
impl Session {
    /// Send graceful shutdown signal (Ctrl+C to PTY process)
    pub async fn send_shutdown_signal(&self) -> Result<()> {
        // Send Ctrl+C to PTY process
        #[cfg(windows)]
        {
            use winapi::um::wincon::GenerateConsoleCtrlEvent;
            use winapi::um::wincon::CTRL_C_EVENT;
            
            unsafe {
                GenerateConsoleCtrlEvent(CTRL_C_EVENT, self.pty.shell_pid());
            }
        }
        
        // Wait for process to exit (up to timeout)
        self.pty.wait_for_exit().await
    }
    
    /// Force kill PTY process (no graceful shutdown)
    pub async fn force_kill(&self) -> Result<()> {
        self.pty.terminate().await
    }
}
```

**Shutdown Timeout:** 10 seconds (not 5 seconds)

**Rationale (per rust-backend-lead):**
- ✅ Gives PTY processes time to flush output
- ✅ Users might have running processes (builds, downloads)
- ✅ 10s is Windows Service default shutdown timeout
- ✅ Matches industry standards (nginx, PostgreSQL use 10s+)

**Phase 1 Trade-off (Accepted):**
- ✅ Force-kill after timeout (simpler, acceptable for MVP)
- ⏳ Phase 2: Make configurable (let users choose graceful-wait vs force-kill)

**Graceful Shutdown Trigger:**
- Service mode: Windows Service Control Manager sends STOP signal
- Console mode: Ctrl+C (SIGINT equivalent on Windows)

---

## Topic 5: Service Control Interface (NEW)

### Decision: Built-In CLI Commands (OPTION A) ✅

**CLI Interface:**

```bash
# Install service (requires admin)
monoterminal --install
# Output: Service installed successfully. Start with: monoterminal --start

# Uninstall service (requires admin)
monoterminal --uninstall
# Output: Service uninstalled successfully.

# Start service (or use Services.msc GUI)
monoterminal --start
# Output: Service started successfully.

# Stop service (or use Services.msc GUI)
monoterminal --stop
# Output: Service stopped successfully.

# Check service status
monoterminal --status
# Output: Service is running (PID: 1234)

# Run in console mode (dev/debug, no service)
monoterminal --console
# Output: Starting MONOTERMINAL in console mode...
#         Press Ctrl+C to exit.
```

**Implementation (using `windows-service` crate):**

```rust
use windows_service::{
    service::{ServiceAccess, ServiceInfo},
    service_manager::{ServiceManager, ServiceManagerAccess},
};

pub fn install_service() -> Result<()> {
    // Check admin privileges
    if !is_elevated()? {
        bail!("ERROR: Service installation requires administrator privileges.\n\
               Please run this command from an elevated prompt.");
    }
    
    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CREATE_SERVICE,
    )?;
    
    let service_binary_path = env::current_exe()?;
    
    let service_info = ServiceInfo {
        name: OsString::from("MONOTERMINAL"),
        display_name: OsString::from("MONOTERMINAL Master Daemon"),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: service_binary_path,
        launch_arguments: vec![],
        dependencies: vec![],
        account_name: None, // LocalService
        account_password: None,
    };
    
    let service = manager.create_service(&service_info, ServiceAccess::CHANGE_CONFIG)?;
    service.set_description("MONOTERMINAL cross-platform terminal multiplexer")?;
    
    // Pre-create directories with ACLs
    setup_service_directories()?;
    
    println!("✅ Service installed successfully.");
    println!("   Start with: monoterminal --start");
    println!("   Or use Services.msc GUI");
    
    Ok(())
}

pub fn uninstall_service() -> Result<()> {
    if !is_elevated()? {
        bail!("ERROR: Service uninstallation requires administrator privileges.");
    }
    
    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT,
    )?;
    
    let service = manager.open_service("MONOTERMINAL", ServiceAccess::DELETE)?;
    service.delete()?;
    
    println!("✅ Service uninstalled successfully.");
    
    Ok(())
}

fn setup_service_directories() -> Result<()> {
    let base_dir = PathBuf::from(env::var("ProgramData")?)
        .join("MONOTERMINAL");
    
    // Create directories
    std::fs::create_dir_all(base_dir.join("certs"))?;
    std::fs::create_dir_all(base_dir.join("logs"))?;
    std::fs::create_dir_all(base_dir.join("data"))?;
    
    // Set ACLs (LocalService needs Write permission)
    set_acl_for_service(&base_dir)?;
    
    Ok(())
}
```

**Service Account:** LocalService (Phase 1)

**Rationale:**
- ✅ Least privilege (no network access needed for Phase 1 local-only)
- ⏳ Phase 2 P2P: May need NetworkService (network access required)

**Alternatives Rejected:**

**Option B: sc.exe only (Windows built-in)**
- ❌ Poor UX: cryptic error messages (`sc create MONOTERMINAL binPath=...`)
- ❌ No configuration validation before install
- ❌ Requires user to manually create directories and set ACLs

**Option C: Services.msc GUI only**
- ❌ Requires manual directory setup
- ❌ No automation for CI/CD or scripted deployments
- ❌ Not suitable for installer integration

**Why OPTION A is superior (per rust-backend-lead):**
- ✅ Configuration validation before install (catch missing certs, invalid paths)
- ✅ Helpful error messages (vs cryptic sc.exe failures)
- ✅ Standard pattern (PostgreSQL, MongoDB, nginx all use built-in commands)
- ✅ Automation-friendly (CI/CD can script `--install`)

---

## Final Decisions (Approved by eng-director, 2026-08-15 21:10)

**All 3 open questions resolved:**

1. **Console mode flag naming:** ✅ **`--console`** (shorter, clearer, approved)

2. **Installer for Phase 1:** ✅ **Option A** - Document manual `monoterminal --install` (Phase 1 MVP)
   - MSI/WiX installer deferred to Phase 2 (production polish)
   - Rationale: Unblocks implementation immediately, MSI adds 1+ week of installer tooling work

3. **Service account:** ✅ **LocalService** (least privilege)
   - Phase 2 P2P: Upgrade to NetworkService if needed (explicit decision point when P2P integration starts)
   - Rationale: Start secure (least privilege), upgrade only if required by P2P architecture

---

## Consequences

### Positive

- ✅ Clean dual-mode support (production service + development console)
- ✅ Health check caching prevents npx spam
- ✅ Graceful shutdown gives processes time to exit
- ✅ Built-in service control improves UX significantly
- ✅ Development doesn't require admin rights

### Negative

- ⚠️ Dual-mode adds complexity (two code paths for paths/logging)
- ⚠️ 10s shutdown timeout might feel slow (but safer than 5s)
- ⚠️ No Windows Event Log in Phase 1 (file logging only)

### Neutral

- Service control CLI adds ~200 LOC (manageable, good UX payoff)
- Health check cache adds state management (Arc\<RwLock\<HealthStatus\>\>)

---

## Implementation Plan

**Phase 1 (task-7 Windows Service - Immediate):**

1. ✅ Implement RunMode detection (Service vs Console)
2. ✅ Implement dual-mode path resolution (get_cert_dir, get_log_dir)
3. ✅ Implement HealthScheduler integration with caching
4. ✅ Implement graceful shutdown (10s timeout, force-kill fallback)
5. ✅ Implement service control CLI (--install/--uninstall/--start/--stop/--console)
6. ✅ Add Windows Service integration via `windows-service` crate
7. ✅ Pre-create directories with ACLs on service install
8. ✅ Document manual installation in README

**Phase 2+ (Future):**
- ⏳ Add Windows Event Log subscriber
- ⏳ Build MSI/WiX installer
- ⏳ Make graceful shutdown timeout configurable
- ⏳ Consider NetworkService account for P2P networking

---

## References

- **SRS §7.1:** Windows Service (Phase 1)
- **SRS §2.4.3:** Monomind Health Check & Upgrade (daily scheduler)
- **Architecture Review 2026-08-15:** principal-architect
- **Feedback 2026-08-15:** rust-backend-lead (health caching, dual-mode, 10s timeout, service CLI)
- **windows-service crate:** https://crates.io/crates/windows-service

---

## Follow-up Actions

1. ✅ **APPROVED 2026-08-15 21:10:** All architectural decisions finalized by eng-director
2. ✅ **IMMEDIATE (devops-lead):** Windows Service implementation starts NOW (task-7, 2-3 day timeline)
3. ⏳ **Before Phase 1 gate:** Test service install/uninstall on clean Windows 10 VM
4. ⏳ **Before Phase 2:** Evaluate NetworkService vs LocalService for P2P networking

---

**Status:** ✅ APPROVED (eng-director, 2026-08-15 21:10)  
**Approved by:** eng-director (nokhodian@gmail.com)  
**Implementation:** UNBLOCKED - devops-lead can start Windows Service (task-7) immediately  
**Next milestone:** Windows Service implementation complete (2-3 days from now)
