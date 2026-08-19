# MONOTERMINAL Protocol Version Compatibility Matrix

**Purpose:** Document which client/server versions can interoperate.

**Policy:** Per ADR-004, update this matrix before releasing any new protocol version.

---

## Version Compatibility Matrix

| Client Version | Server v1.0 | Server v1.1 | Server v2.0 |
|----------------|-------------|-------------|-------------|
| **v1.0**       | ✅ Full      | ✅ Downgrade | ❌ Reject    |
| **v1.1**       | ✅ Downgrade | ✅ Full      | ❌ Reject    |
| **v2.0**       | ❌ Reject    | ❌ Reject    | ✅ Full      |

**Legend:**
- ✅ **Full**: All features work (matching protocol versions)
- ✅ **Downgrade**: Connection succeeds, new features unavailable (version negotiation)
- ❌ **Reject**: Connection fails with `INCOMPATIBLE_VERSION` error

---

## Current Released Versions

### v1.0 (Phase 1 MVP - Current)

**Release Date:** 2026-08-15  
**Client Versions:** 0.1.0+  
**Server Versions:** 0.1.0+

**Features:**
- Session management (attach, detach)
- Terminal I/O (input, output, resize)
- Error handling
- Monomind integration (dashboard, health check, upgrade, detection, monitoring)

**Limitations:**
- No version negotiation (assumes all peers are v1.0)
- No compression (CompressionType::ZSTD defined but not implemented)
- No P2P networking (Phase 2)

**Upgrade Path:** Direct upgrade to v1.1 (backward compatible)

---

## Planned Versions

### v1.1 (Phase 1.5 - Before Phase 2 P2P)

**Planned Release:** TBD (before Phase 2 ships)  
**Client Versions:** 0.2.0+  
**Server Versions:** 0.2.0+

**New Features:**
- Version negotiation (protocol_version field in Envelope)
- Compatibility detection (AttachRequest/AttachResponse exchange versions)
- `INCOMPATIBLE_VERSION` error code

**Backward Compatibility:**
- ✅ v1.1 clients can talk to v1.0 servers (downgrades to v1.0)
- ✅ v1.0 clients can talk to v1.1 servers (server detects missing version field)
- Field additions only (no breaking changes)

**Upgrade Strategy:**
1. **Server-first upgrade**: Deploy v1.1 servers, v1.0 clients continue working
2. **Client rollout**: Gradually upgrade clients to v1.1 (no forced upgrade)
3. **Full v1.1**: Once 95%+ clients upgraded, new features can be enabled

**Version Negotiation Flow:**

```
v1.0 Client                    v1.1 Server
     │                              │
     ├─ AttachRequest ──────────────►│
     │  (no protocol_version)        │
     │                               │ Detects: version=0 (v1.0 client)
     │◄────────── AttachResponse ────┤
     │  (protocol_version=0)          │ Responds: version=0 (v1.0 compat)
     │                               │
     │   All messages use v1.0       │
```

```
v1.1 Client                    v1.1 Server
     │                              │
     ├─ AttachRequest ──────────────►│
     │  (protocol_version=1)         │
     │                               │ Detects: version=1 (v1.1 client)
     │◄────────── AttachResponse ────┤
     │  (protocol_version=1)          │ Responds: version=1 (v1.1 full)
     │                               │
     │   All messages use v1.1       │
```

---

### v2.0 (Future - Breaking Changes)

**Planned Release:** TBD (not before Phase 3)  
**Trigger:** First breaking change (field number reuse, required field change, package rename)

**Breaking Changes:** TBD (none identified yet)

**Migration Path:**
1. **Dual-stack period (6-12 months):**
   - Server supports BOTH v1.x and v2.0 (separate protobuf packages)
   - Clients choose version via AttachRequest
2. **Deprecation notice:**
   - v1.x marked deprecated 6 months before v2.0-only
   - Warning message sent to v1.x clients
3. **v1.x sunset:**
   - v1.x support dropped when <5% clients remain on old version

**Backward Compatibility:**
- ❌ v2.0 clients CANNOT talk to v1.x servers (breaking changes)
- ❌ v1.x clients CANNOT talk to v2.0-only servers (after dual-stack period ends)

---

## Version Mapping to Features

| Feature | Min Client | Min Server | Protocol Version |
|---------|-----------|-----------|------------------|
| **Core Session Management** | 0.1.0 | 0.1.0 | v1.0 |
| Attach/Detach | 0.1.0 | 0.1.0 | v1.0 |
| Terminal I/O (input/output/resize) | 0.1.0 | 0.1.0 | v1.0 |
| Error responses | 0.1.0 | 0.1.0 | v1.0 |
| **Monomind Integration** | 0.1.0 | 0.1.0 | v1.0 |
| Dashboard (org/agent status) | 0.1.0 | 0.1.0 | v1.0 |
| Health check (monomind doctor) | 0.1.0 | 0.1.0 | v1.0 |
| Upgrade (one-click monomind upgrade) | 0.1.0 | 0.1.0 | v1.0 |
| Detection (per-session .monomind/ check) | 0.1.0 | 0.1.0 | v1.0 |
| Monitoring (live org metrics stream) | 0.1.0 | 0.1.0 | v1.0 |
| **Version Negotiation** | 0.2.0 | 0.2.0 | v1.1 |
| protocol_version field | 0.2.0 | 0.2.0 | v1.1 |
| Compatibility detection | 0.2.0 | 0.2.0 | v1.1 |
| **Compression** | TBD | TBD | v1.x or v2.0 |
| ZSTD compression (defined, not impl) | TBD | TBD | TBD |
| **P2P Networking** | TBD | TBD | v1.x or v2.0 |
| WebRTC DataChannel | TBD | TBD | Phase 2 |
| Peer discovery | TBD | TBD | Phase 2 |

---

## Deprecation Policy

**Minor version deprecation (v1.x → v1.y):**
- No forced upgrades (backward compatible)
- Old features continue working indefinitely
- New features may require newer client/server

**Major version deprecation (v1.x → v2.0):**
1. **Announcement**: 12 months before v2.0 release
2. **Dual-stack**: 6-12 months (both v1.x and v2.0 supported)
3. **Deprecation warnings**: v1.x clients see upgrade prompt
4. **Sunset**: v1.x support dropped when <5% usage remains

**Enterprise support:**
- LTS releases get extended support (Phase 4+)
- Security patches backported to previous major version for 2 years

---

## Testing Requirements

**Before releasing new protocol version:**

1. ✅ **Schema evolution tests** pass (ADR-004 requirement)
   - Old client + new server
   - New client + old server
   - Unknown field handling
   - Unknown message type handling

2. ✅ **Compatibility matrix validation**
   - All green checkmarks in matrix above tested in CI
   - Red X rejection cases tested (correct error message)

3. ✅ **Documentation updated**
   - PROTOCOL_CHANGELOG.md entry
   - This compatibility matrix updated
   - SRS version references updated

**CI enforcement:**
- `cargo test -p monoterminal-protocol` must pass
- Integration tests cover version negotiation (v1.1+)

---

## How to Add a New Version

**For backward-compatible additions (v1.x → v1.y):**

1. Update `proto/monoterminal/v1/messages.proto`:
   - Add new fields with unique field numbers (never reuse!)
   - Add new message types to `Envelope.oneof`
   - Add new enum values (with new numbers)

2. Update this file:
   - Add new row to compatibility matrix
   - Add feature to version mapping table
   - Update "Current Released Versions" section

3. Update `PROTOCOL_CHANGELOG.md`:
   - Document all changes
   - Note backward compatibility status
   - Provide migration notes

4. Create schema evolution tests:
   - Test old client with new feature (ignores gracefully)
   - Test new client with old server (degrades gracefully)

**For breaking changes (v1.x → v2.0):**

1. Create new package `proto/monoterminal/v2/messages.proto`
2. Announce deprecation timeline (12 months minimum)
3. Implement dual-stack support in server
4. Follow deprecation policy above

---

## References

- **ADR-004:** Protocol Schema Evolution Policy
- **SRS §3.1.1:** Protocol Buffers Schema
- **PROTOCOL_CHANGELOG.md:** Detailed change history
- **Protobuf file:** `proto/monoterminal/v1/messages.proto`

---

**Maintained by:** principal-architect  
**Last updated:** 2026-08-15  
**Next review:** Before each protocol version release
