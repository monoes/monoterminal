# ADR-001: Rust From Scratch (Not Fork Ghostty/WezTerm)

**Status:** Accepted  
**Date:** 2026-08-13  
**Deciders:** product-owner, principal-architect  
**SRS Reference:** §8.1.3

---

## Context

MONOTERMINAL requires a cross-platform terminal emulator master daemon with:
- PTY management (Linux/macOS/Windows)
- GPU-accelerated rendering
- P2P networking (WebRTC)
- Multi-platform client support

We evaluated forking existing projects vs. building from scratch in Rust.

---

## Decision

**Build from scratch in Rust**, using Ghostty/WezTerm as reference implementations only (not forks).

---

## Alternatives Considered

### Option A: Fork Ghostty

**Pros:**
- Proven VT parser (~2k LOC)
- Metal rendering pipeline
- MIT license (permissive)

**Cons:**
- Written in Zig (immature mobile support as of Jan 2025)
- No multiplexer architecture (local-only)
- 70% of MONOTERMINAL is networking/P2P (greenfield anyway)
- No Android NDK support in Zig

**Verdict:** ❌ Rejected

---

### Option B: Fork WezTerm

**Pros:**
- Mature Rust multiplexer
- wgpu rendering (cross-platform)
- Proven PTY abstraction

**Cons:**
- Local-first architecture (mux server is SSH-based)
- 60% of code would need rewriting for P2P
- Complex existing architecture to untangle

**Verdict:** ❌ Rejected

---

### Option C: Fork Alacritty

**Pros:**
- Excellent GPU renderer (wgpu)
- Clean Rust codebase

**Cons:**
- NO multiplexing at all (pure local terminal)
- 80% of MONOTERMINAL is multiplexer + networking (greenfield)

**Verdict:** ❌ Rejected

---

### Option D: Rust From Scratch ✅

**Pros:**
- Full control of architecture
- P2P-native design from day 1
- wgpu ecosystem (Metal/Vulkan/DX12 in one codebase)
- tokio async for networking (mature, integrates with webrtc-rs)
- 70% is greenfield anyway (networking, protocol, mobile)

**Cons:**
- Slower MVP (3 months vs 1-2 if forked)
- Need to re-implement VT parser (~2k LOC)
- Need to re-implement PTY abstraction (~1k LOC)

**Trade-offs Accepted:**
- ✅ Use `vte` crate as reference for VT parser
- ✅ Use `portable-pty` crate as reference for PTY abstraction
- ✅ Learn from Ghostty's Metal rendering patterns
- ✅ Learn from WezTerm's session management

**Verdict:** ✅ **CHOSEN**

---

## Consequences

### Positive

- Clean architecture designed for P2P from the start
- No legacy code to work around
- Rust ecosystem advantages (wgpu, tokio, cargo)
- Single language for all platforms (Rust)

### Negative

- 3-month MVP instead of 1-2 months
- Need to implement VT parser and PTY abstraction
- Higher upfront learning curve for Rust

### Neutral

- Can still reference Ghostty/WezTerm code for patterns
- ~50k LOC estimated (reasonable for 9-12 month timeline)

---

## References

- SRS §8.1.3 (Rust Rewrite: Hybrid Approach)
- Ghostty: github.com/ghostty-org/ghostty (MIT)
- WezTerm: github.com/wez/wezterm (MIT + Apache-2.0)
- Alacritty: github.com/alacritty/alacritty (Apache-2.0)

---

## Follow-up Actions

1. ✅ Set up Cargo workspace (crates/master, crates/protocol, crates/monomind-bridge)
2. ⏳ Research `vte` crate for VT parsing
3. ⏳ Research `portable-pty` crate for PTY abstraction patterns
4. ⏳ Study Ghostty's Metal rendering for wgpu patterns
