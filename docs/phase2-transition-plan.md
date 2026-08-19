# Phase 2 Transition Plan - DRAFT

**Status:** PENDING Phase 1 Gate Passage (5/7)  
**Expected Start:** Tuesday Aug 19, 2026  
**Duration:** 3 months (SRS §7.2)  
**Goal:** P2P networking + Multi-session + Persistence + Collaboration  

---

## Executive Summary

Upon achieving 5/7 Phase 1 gate passage, MONOTERMINAL transitions to Phase 2 with two parallel tracks:

**Track A (Phase 2 SRS Features):**
- P2P WebRTC networking
- Multi-session management
- SQLite persistence
- Multi-client collaboration

**Track B (Phase 1 Completion):**
- Coverage improvement (41% → 70%)
- Memory leak validation + 24h soak test

**Both tracks required for full Phase 2 acceptance criteria (SRS §7.2)**

---

## Phase 1 Carryover Work

### Priority 1: Coverage Improvement (41% → 70%)

**Current Status:** 41.03% (1,040/2,535 lines)  
**Target:** ≥70% per SRS §7.1  
**Gap:** +28.97 percentage points needed

**Root Cause Identified:**
- UI/renderer layer: 0% coverage (614 lines = 24% of codebase)
- Reason: GPU context requirement (wgpu/egui headless backend needed)

**Implementation Plan:**

**Week 1-2 (Quick Wins): +15-20% → 56-61%**
1. Headless renderer tests (+10-12%)
   - Use wgpu headless backend
   - Cover renderer init, pipeline setup
   - Target: renderer.rs coverage

2. Main entry point smoke test (+3-4%)
   - Startup/shutdown sequence
   - Config loading validation

3. Server handler protocol tests (+5-8%)
   - WebSocket message handlers
   - Protocol state machine coverage

**Week 3-4 (Medium-Term): +10-15% → 66-76%**
4. UI integration framework (+8-10%)
   - Headless window creation
   - Font loading, layout tests

5. Protocol round-trip tests (+2-3%)
   - Protobuf serialization coverage

**Owner:** test-engineer-unit (task-5)  
**Timeline:** 2-3 weeks  
**Success Metric:** ≥70% workspace coverage

---

### Priority 2: Memory Leak Fix Validation + 24h Soak Test

**Current Status:** Fix implemented, verification pending  
**Root Cause:** Fire-and-forget tokio tasks holding Arc references  
**Fix:** AbortOnDrop pattern (commit 6788108)

**Validation Timeline:**

**Monday Aug 18:**
- 09:00: Compile + unit tests
- 09:30: 5-minute smoke test (target: <10% memory growth)
- 10:00: Results analysis

**Monday Aug 18 (if 5-min passes):**
- 10:30: 1-hour smoke test launch
- 11:30: Results analysis (target: <10% growth sustained)

**Wednesday Aug 19 (if 1-hour passes):**
- 09:00: 24-hour soak test launch
- Monitoring: Memory, CPU, handles (1-min sampling)
- Alerts: Threshold violations logged

**Thursday Aug 20:**
- 09:00: Soak test completion
- Evidence collection: SUMMARY.json, metrics CSV, alerts log
- Criterion #7 verification

**Owner:** rust-backend-lead (task-2) → performance-engineer (soak execution)  
**Timeline:** Wed-Thu (4 days from Monday)  
**Success Metric:** Zero crashes, <10% memory growth over 24 hours

---

## Phase 2 SRS Features (Track A)

### Feature 1: P2P WebRTC Networking (SRS §2.3.1)

**Current State:** Direct WebSocket only (Phase 1)  
**Target State:** P2P peer connections + hybrid discovery

**Implementation Components:**

**1. WebRTC Stack Integration**
- Library: rust-webrtc (tokio-based)
- Protocols: STUN/TURN for NAT traversal
- Transport: DTLS + SRTP

**2. Discovery Layer**
- Local: mDNS for same-LAN peers
- Internet: Directory service for remote peers
- Hybrid: Fallback from P2P to relay

**3. Connection Negotiation**
- SDP offer/answer exchange
- ICE candidate gathering
- Connection establishment + fallback

**Work Estimate:**
- WebRTC integration: 2 weeks
- Discovery implementation: 1 week
- Testing + NAT traversal validation: 1 week
- **Total: 4 weeks**

**Owner:** networking-engineer  
**Priority:** HIGH (enables Phase 2 core value)

**Acceptance Criteria (SRS §7.2):**
- 65-80% NAT traversal success (real network conditions)
- P2P connection establishes <5 seconds
- Fallback to relay if P2P fails

---

### Feature 2: Multi-Session Management (SRS §7.2)

**Current State:** Single session per master (Phase 1)  
**Target State:** 100 concurrent sessions per master

**Implementation Components:**

**1. Session Multiplexer**
- Session registry (HashMap<SessionId, Session>)
- Create/list/kill API
- Resource isolation per session

**2. Protocol Extensions**
- CreateSessionRequest/Response
- ListSessionsRequest/Response
- KillSessionRequest/Response
- Session routing in protocol handlers

**3. Resource Management**
- PTY handle pooling
- Memory limits per session
- CPU time quotas

**Work Estimate:**
- Session registry: 1 week
- Protocol extensions: 1 week
- Resource management: 1 week
- Testing (100 concurrent): 1 week
- **Total: 4 weeks**

**Owner:** rust-backend-lead  
**Priority:** HIGH (blocks collaboration)

**Acceptance Criteria (SRS §7.2):**
- 100 concurrent sessions tested
- Session isolation verified
- No cross-session leaks

---

### Feature 3: SQLite Persistence (SRS §4.1)

**Current State:** In-memory only (10k line scrollback)  
**Target State:** SQLite with zstd compression

**Implementation Components:**

**1. Database Schema**
- Tables: sessions, clients, scrollback, session_permissions, audit_log
- Indexes: sessions(owner_uid), clients(session_id), scrollback(session_id, line_number)

**2. Persistence Layer**
- Session state save/restore
- Scrollback overflow to disk
- Connection recovery on restart

**3. Compression**
- zstd for scrollback blobs
- Compression ratio target: 3-5x

**Work Estimate:**
- Schema implementation: 1 week
- Persistence layer: 2 weeks
- Compression integration: 1 week
- Migration + testing: 1 week
- **Total: 5 weeks**

**Owner:** rust-engineer-storage  
**Priority:** MEDIUM (quality-of-life, not core P2P)

**Acceptance Criteria (SRS §7.2):**
- Session persistence across restarts
- Scrollback >10k lines stored
- Compression ratio ≥3x

---

### Feature 4: Multi-Client Collaboration (SRS §7.2)

**Current State:** Single client attach (Phase 1)  
**Target State:** Multi-client with presence + input control

**Implementation Components:**

**1. Multi-Client Attach**
- Client registry per session
- Output fan-out to all attached clients
- Presence notifications (ClientJoined, ClientLeft)

**2. Input Broadcasting**
- Shared input mode (all clients can type)
- Exclusive input mode (one client controls)
- Input lock request/grant protocol

**3. Cursor + Viewport Sharing**
- CursorPosition broadcasts
- ScrollViewport synchronization
- Visual indicators for other users

**Work Estimate:**
- Multi-client attach: 1 week
- Input broadcasting: 2 weeks
- Cursor/viewport sharing: 1 week
- Testing + UX refinement: 1 week
- **Total: 5 weeks**

**Owner:** rust-backend-lead + frontend-lead  
**Priority:** MEDIUM (extends P2P value)

**Acceptance Criteria (SRS §7.2):**
- Multiple clients attach simultaneously
- Input control switching works
- Presence indicators functional

---

## Phase 2 Timeline (12 Weeks)

### Weeks 1-4: Foundation + P2P
**Parallel Work:**
- Track A: WebRTC integration (networking-engineer)
- Track A: Multi-session management (rust-backend-lead)
- Track B: Coverage improvement (test-engineer-unit)
- Track B: Memory leak validation (rust-backend-lead → performance-engineer)

**Milestones:**
- Week 2: Coverage ≥56% (quick wins)
- Week 3: WebRTC P2P connections working
- Week 4: 24h soak test PASS (Criterion #7 complete)

---

### Weeks 5-8: Persistence + Collaboration
**Parallel Work:**
- Track A: SQLite persistence (rust-engineer-storage)
- Track A: Multi-client collaboration (rust-backend-lead + frontend-lead)
- Track B: Coverage ≥70% (test-engineer-unit)

**Milestones:**
- Week 6: Coverage ≥70% (Criterion #6 complete)
- Week 7: Multi-session 100 concurrent tested
- Week 8: Session persistence working

---

### Weeks 9-12: Integration + Validation
**Integration Work:**
- End-to-end P2P + multi-session + persistence flow
- Collaboration UX refinement
- Performance testing (SRS §7.2 acceptance)

**Acceptance Criteria Verification:**
- 100 concurrent sessions ✅
- 65-80% NAT traversal ✅
- iOS backgrounding <10s reconnect ✅
- 75% test coverage ✅

**Milestones:**
- Week 10: Integration testing complete
- Week 11: Phase 2 acceptance criteria verified
- Week 12: Phase 2 → Phase 3 gate passage

---

## Resource Allocation

### Engineering Team (Phase 2)
**Backend:**
- rust-backend-lead: Multi-session, collaboration (6 weeks)
- rust-engineer-storage: SQLite persistence (5 weeks)
- networking-engineer: WebRTC P2P (4 weeks)

**Frontend:**
- frontend-lead: Collaboration UI (2 weeks)
- frontend-engineer: P2P connection management (2 weeks)

**QA:**
- test-engineer-unit: Coverage improvement (3 weeks)
- performance-engineer: Soak test + 100-session testing (2 weeks)
- test-engineer-e2e: P2P + collaboration E2E (3 weeks)

**Total Effort:** ~27 person-weeks over 12 weeks (2.25 FTE average)

---

## Risk Register

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| WebRTC NAT traversal <65% | HIGH | MEDIUM | Implement relay fallback, test on real networks early |
| Memory leak recurs under load | HIGH | LOW | 100-session testing, memory profiling, continuous monitoring |
| Coverage infrastructure complexity | MEDIUM | MEDIUM | Headless GPU backend research, allocate 2-3 weeks |
| SQLite performance bottleneck | MEDIUM | LOW | Benchmark early, consider async SQLite (tokio-rusqlite) |
| Collaboration UX confusing | MEDIUM | MEDIUM | User testing, iterative refinement |

---

## Dependencies & Blockers

**Phase 1 Gate Passage:**
- ⚠️ BLOCKS Phase 2 start
- **Requirement:** 5/7 criteria verified
- **Status:** 4/7 complete, Criterion #5 Monday
- **Risk:** Low (75-85% probability)

**Memory Leak Fix:**
- ⚠️ BLOCKS Criterion #7 soak test
- **Requirement:** Fix verification + smoke test pass
- **Status:** Fix implemented, Monday verification
- **Risk:** Low (targeted fix, smoke test validation)

**Coverage Infrastructure:**
- ⚠️ EXTENDS Phase 2 timeline if complex
- **Requirement:** Headless GPU backend for wgpu
- **Status:** Research needed
- **Risk:** Medium (unknown complexity)

---

## Success Metrics (Phase 2 End)

**Technical:**
- ✅ 100 concurrent sessions (tested)
- ✅ 65-80% NAT traversal (real networks)
- ✅ <10s reconnect on iOS background
- ✅ 75% test coverage
- ✅ 24h soak test PASS

**Quality:**
- ✅ P2P connection <5s establishment
- ✅ SQLite compression ≥3x
- ✅ Multi-client collaboration functional
- ✅ Zero crashes in 24h soak

**Readiness:**
- ✅ Phase 3 (Linux/macOS) unblocked
- ✅ All Phase 1 criteria complete (7/7)
- ✅ Production stability validated

---

## Phase 2 → Phase 3 Transition

**Upon Phase 2 Completion:**
- Phase 3: Linux + macOS master support (SRS §7.3)
- Phase 4: Enterprise features (SSO, audit, SLA) (SRS §7.4)

**Estimated Timeline:**
- Phase 2: 3 months (Aug-Nov 2026)
- Phase 3: 3 months (Nov 2026-Feb 2027)
- Phase 4: 3 months (Feb-May 2027)
- **Total to v1.0:** 12 months from Phase 1 start

---

**Prepared by:** eng-director  
**Draft Date:** 2026-08-17  
**Status:** DRAFT - Pending Phase 1 gate passage  
**Next Review:** Tuesday Aug 19 (upon 5/7 achievement)
