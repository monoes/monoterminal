# Phase 3: Unix PTY Adapter Design

**Date:** 2026-08-19  
**Author:** rust-backend-lead  
**Status:** Pre-work (Week 0, parallel with CI setup)  
**Target:** Week 1-2 implementation

## Overview

Design for Unix PTY backend using `portable-pty` crate as the underlying implementation, wrapped in our `PtyBackend` trait for cross-platform abstraction.

## Goals

1. Implement `PtyBackend` trait for Linux/macOS
2. Reuse `portable-pty` crate (proven, maintained, cross-platform)
3. Match Windows ConPTY patterns for consistency
4. ~500 LOC adapter layer (per ADR-015)

## PtyBackend Trait Requirements

```rust
#[async_trait]
pub trait PtyBackend: Send + Sync {
    /// Create PTY session with config
    async fn create(config: PtyConfig) -> PtyResult<Self> where Self: Sized;
    
    /// Read output from PTY (non-blocking, returns 0 on EOF)
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    
    /// Write input to PTY (flushes immediately)
    async fn write(&mut self, data: &[u8]) -> io::Result<()>;
    
    /// Resize PTY dimensions
    fn resize(&mut self, rows: u16, cols: u16) -> PtyResult<()>;
    
    /// Get shell process ID
    fn shell_pid(&self) -> u32;
    
    /// Terminate PTY session (kill process, cleanup)
    async fn terminate(self: Box<Self>) -> PtyResult<()>;
}
```

## Adapter Architecture

### 1. UnixPtyBackend Structure

```rust
// crates/master/src/pty/unix.rs

pub struct UnixPtyBackend {
    /// portable-pty PtyPair (master + slave)
    pty_pair: portable_pty::PtyPair,
    
    /// Child process handle
    child: portable_pty::Child,
    
    /// Buffered output reader (4KB buffer per SRS §3.1.4)
    output_reader: BufReader<PtyReader>,
    
    /// Direct input writer (unbuffered for immediate delivery)
    input_writer: PtyWriter,
    
    /// Shell process ID
    shell_pid: u32,
}
```

### 2. portable-pty Mapping

**Expected portable-pty API** (based on standard Rust PTY library patterns):

| portable-pty | Our Adapter | Notes |
|--------------|-------------|-------|
| `PtySystem::native()` | `UnixPtyBackend::create()` | Get native PTY implementation |
| `PtySystem::openpty(size)` | Internal setup | Create PTY pair with size |
| `PtyPair::master` | `output_reader` + `input_writer` | Master FD for I/O |
| `PtyPair::slave` | Passed to child | Slave FD for child process |
| `CommandBuilder::new(cmd)` | From `PtyConfig::shell` | Build shell command |
| `CommandBuilder::spawn()` | Create child process | Fork/exec shell |
| `Child::process_id()` | `shell_pid()` | Get PID |
| `Child::kill()` | `terminate()` | Kill process |
| `MasterPty::resize()` | `resize()` | Window size change |

### 3. Async I/O Strategy

**Same pattern as ConPTY** (proven in Phase 1):

```rust
impl AsyncRead for PtyReader {
    fn poll_read(...) -> Poll<io::Result<usize>> {
        // Use spawn_blocking for synchronous read
        // portable-pty likely provides blocking I/O
        tokio::task::spawn_blocking(move || {
            // Read from master PTY fd
            pty_master.read(buf)
        }).await
    }
}

impl AsyncWrite for PtyWriter {
    fn poll_write(...) -> Poll<io::Result<usize>> {
        // Unbuffered write for immediate delivery
        tokio::task::spawn_blocking(move || {
            pty_master.write(data)
        }).await
    }
}
```

**Rationale:** Simple, proven approach from ConPTY. Phase 3+ can optimize with tokio-uring for Linux.

### 4. PtyConfig Translation

```rust
async fn create(config: PtyConfig) -> PtyResult<Self> {
    // 1. Get native PTY system
    let pty_system = portable_pty::native_pty_system();
    
    // 2. Create PTY pair with dimensions
    let pty_pair = pty_system.openpty(PtySize {
        rows: config.rows,
        cols: config.cols,
        pixel_width: 0,  // Not used
        pixel_height: 0, // Not used
    })?;
    
    // 3. Build command from config
    let mut cmd = CommandBuilder::new(config.shell);
    cmd.cwd(config.working_dir);
    cmd.env_clear(); // Start clean
    for (key, val) in config.environment {
        cmd.env(key, val);
    }
    
    // 4. Spawn child process with slave PTY
    let child = pty_pair.slave.spawn_command(cmd)?;
    let shell_pid = child.process_id().unwrap_or(0);
    
    // 5. Wrap I/O in async readers/writers (4KB buffer)
    let output_reader = BufReader::with_capacity(
        4096, // SRS §3.1.4
        PtyReader::new(pty_pair.master.try_clone_reader()?)
    );
    let input_writer = PtyWriter::new(
        pty_pair.master.try_clone_writer()?
    );
    
    Ok(UnixPtyBackend {
        pty_pair,
        child,
        output_reader,
        input_writer,
        shell_pid,
    })
}
```

### 5. ConPTY Patterns to Replicate

From `crates/master/src/pty/conpty.rs` analysis:

**Pattern 1: 4KB Buffer**
```rust
const PTY_BUFFER_SIZE: usize = 4096; // SRS §3.1.4
output_reader: BufReader::with_capacity(PTY_BUFFER_SIZE, ...)
```

**Pattern 2: Unbuffered Input**
```rust
// No buffering on input writer - immediate delivery
input_writer: PtyWriter::new(...)
```

**Pattern 3: Drop Cleanup**
```rust
impl Drop for UnixPtyBackend {
    fn drop(&mut self) {
        // Best-effort cleanup
        let _ = self.child.kill();
    }
}
```

**Pattern 4: Terminate Semantics**
```rust
async fn terminate(mut self: Box<Self>) -> PtyResult<()> {
    // 1. Kill child process
    self.child.kill()?;
    
    // 2. Wait for process exit (with timeout)
    tokio::time::timeout(
        Duration::from_secs(5),
        self.child.wait()
    ).await??;
    
    // 3. Close PTY master (Drop handles this)
    Ok(())
}
```

## Platform-Specific Compilation

```rust
// crates/master/src/pty/mod.rs

#[cfg(windows)]
pub mod conpty;

#[cfg(unix)]  // NEW
pub mod unix;

#[cfg(windows)]
pub use conpty::ConPtyBackend;

#[cfg(unix)]  // NEW
pub use unix::UnixPtyBackend;
```

## Unix-Specific Considerations

### 1. Pseudoterminal Setup (Linux/macOS)

**Linux:** Uses `posix_openpt()` + `grantpt()` + `unlockpt()` + `ptsname()`
- `portable-pty` abstracts this complexity
- Master/slave FD pair created
- Slave FD passed to child via environment

**macOS:** Uses BSD `openpty()` system call
- Similar semantics, different API
- `portable-pty` handles platform differences

### 2. Fork/Exec Pattern

Unix PTY spawning:
1. Create PTY pair (master + slave)
2. Fork process
3. In child: Close master, dup2 slave to stdin/stdout/stderr
4. In child: Exec shell
5. In parent: Close slave, keep master for I/O

**portable-pty handles this** - we just call `spawn_command()`

### 3. Signal Handling

**Window size changes (SIGWINCH):**
```rust
fn resize(&mut self, rows: u16, cols: u16) -> PtyResult<()> {
    self.pty_pair.master.resize(PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    })?;
    Ok(())
}
```

**Process termination (SIGCHLD):**
- `portable-pty::Child::wait()` handles this
- Called in `terminate()`

### 4. Environment Variables

Unix shells expect specific environment:
- `TERM=xterm-256color` (or similar)
- `SHELL=/bin/bash` (or user's shell)
- `HOME=/home/user`
- `USER=username`

**From PtyConfig:**
```rust
cmd.env("TERM", "xterm-256color");
for (key, val) in config.environment {
    cmd.env(key, val);
}
```

## Implementation Checklist

- [ ] Add `portable-pty` to `Cargo.toml` (Linux/macOS only)
- [ ] Create `crates/master/src/pty/unix.rs`
- [ ] Implement `UnixPtyBackend` struct
- [ ] Implement `PtyBackend` trait
- [ ] Add platform guards (`#[cfg(unix)]`) to `mod.rs`
- [ ] Write unit tests (Linux/macOS)
- [ ] Test on Ubuntu 22.04 LTS
- [ ] Test on macOS 13+ (Ventura)
- [ ] Update CI to run Unix PTY tests
- [ ] Documentation updates

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_unix_pty_create() {
        let config = PtyConfig {
            rows: 24,
            cols: 80,
            shell: "/bin/sh".to_string(),
            working_dir: PathBuf::from("/tmp"),
            environment: HashMap::new(),
        };
        
        let mut pty = UnixPtyBackend::create(config).await.unwrap();
        assert!(pty.shell_pid() > 0);
    }
    
    #[tokio::test]
    async fn test_unix_pty_read_write() {
        // Create PTY, write command, read output
        // Similar to ConPTY tests
    }
    
    #[tokio::test]
    async fn test_unix_pty_resize() {
        // Verify resize works without error
    }
}
```

### Integration Tests

- Session creation with Unix PTY
- Input/output flow
- Resize operations
- Process termination
- Error handling (invalid shell, permission denied)

## Estimated LOC

Based on ConPTY implementation (~600 LOC):

- Struct definition: ~20 LOC
- `create()`: ~80 LOC
- `read()`/`write()`: ~60 LOC
- `resize()`: ~10 LOC
- `terminate()`: ~30 LOC
- Async I/O wrappers: ~150 LOC
- Drop impl: ~10 LOC
- Unit tests: ~150 LOC

**Total:** ~510 LOC (matches ADR-015 estimate of ~500 LOC)

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| portable-pty API different than expected | High | Review actual API first, adjust adapter design |
| Async I/O performance | Medium | Use spawn_blocking (proven), optimize later with tokio-uring |
| Platform differences (Linux vs macOS) | Medium | Extensive testing on both platforms |
| Signal handling edge cases | Low | portable-pty handles this, test thoroughly |

## Dependencies

- **Week 0 CI** (devops-lead): Needed for Linux/macOS testing
- **portable-pty crate**: Verify latest version supports async patterns
- **tokio**: Already used for async runtime

## Success Criteria

1. Unix PTY backend implements `PtyBackend` trait
2. All unit tests pass on Linux + macOS
3. SessionManager works with Unix PTY (no code changes needed)
4. Performance matches Windows ConPTY (within 10%)
5. Cross-platform abstraction verified (trait works on all platforms)

## Next Steps (Week 1 Implementation)

1. Add `portable-pty` dependency
2. Implement `UnixPtyBackend` struct + trait
3. Write comprehensive tests
4. Validate on Linux CI
5. Validate on macOS CI
6. Performance benchmarking
7. Documentation

---

**Design Status:** Pre-work complete, ready for Week 1 implementation  
**Blocker:** None (CI setup in parallel)  
**Risk Level:** Low (proven approach, library-based)
