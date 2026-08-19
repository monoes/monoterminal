# Phase 2 Kickoff Brief - DRAFT
**Status:** CONTINGENT on Criterion #5 fix completion (ETA 00:15)  
**Date:** 2026-08-17 (prepared 23:20)  
**Authority:** eng-director  

---

## Trigger Condition

**This document activates ONLY IF:**
- ✅ rust-backend-lead's RwLock fix completes successfully
- ✅ Criterion #5 benchmark passes (p95 < 10ms)
- ✅ qa-lead verifies fix quality + benchmark results
- ✅ Gate status reaches 5/7 (71%) per ADR-006

**If trigger condition NOT met:** Defer to Tuesday 09:00 war room (qa-lead contingency plan)

---

## Phase 1 → Phase 2 Transition

### Phase 1 Status (Upon 5/7 Achievement)

**Verified Criteria (5/7):**
1. ✅ 60 FPS rendering
2. ✅ Mobile E2E (PWA on iOS/Android)
3. ✅ Monomind detection
4. ✅ Embedded dashboard
5. ✅ <10ms LAN latency (p95)

**Deferred to Phase 2 (2/7):**
6. ⚠️ 70% test coverage - Currently 41% (Track B completion work)
7. ⚠️ 24h soak test - Blocked by memory leak (Track B validation work)

**Gate Passage:** 5/7 (71%) meets ADR-006 minimum threshold ✅

---

## Phase 2 Scope (SRS §7.2)

Per docs/phase2-transition-plan.md, Phase 2 has two parallel tracks:

### Track A: Phase 2 SRS Features (NEW FUNCTIONALITY)

**Week 1-2: P2P Foundation**
- WebRTC peer-to-peer networking (networking-engineer)
- Directory service for peer discovery (principal-architect)
- Multi-session management (rust-backend-lead)

**Week 3-4: Persistence & Collaboration**
- SQLite session persistence (rust-engineer-storage)
- Multi-client collaboration (rust-backend-lead + frontend-lead)
- Conflict-free state synchronization (principal-architect)

**Week 5-6: Production Hardening**
- Connection resilience (reconnect, offline mode)
- Performance optimization (bandwidth, CPU)
- Security audit (P2P auth model)

### Track B: Phase 1 Completion (CARRYOVER WORK)

**Criterion #6: Test Coverage (41% → 70%)**
- Quick wins: 2-3 days → 56-61% coverage
- Full path: 1 week → 66-76% coverage
- Requires: Headless GPU backend integration (gpu-rendering-engineer)
- Owner: test-engineer-unit

**Criterion #7: Memory Leak + Soak Test**
- Memory leak fix validation (5-min smoke test)
- 1-hour smoke test (if 5-min passes)
- 24-hour soak execution (Wednesday-Thursday)
- Owner: devops-lead + sre-observability-engineer

---

## Immediate Next Steps (If 5/7 Achieved Tonight)

### Tuesday Morning (2026-08-18 09:00)

**1. Phase 2 Architecture Design Session**
- **Owner:** principal-architect
- **Attendees:** rust-backend-lead, networking-engineer, security-engineer
- **Agenda:**
  - WebRTC signaling architecture
  - Directory service design (centralized vs DHT)
  - P2P protocol message inventory (17 new message types from recall)
  - Session state synchronization model

**2. Track B Planning**
- **Coverage:** test-engineer-unit + gpu-rendering-engineer (headless backend scope)
- **Soak test:** devops-lead + rust-backend-lead (memory leak smoke test schedule)

**3. Protocol Schema Evolution Review**
- **Owner:** principal-architect
- **Scope:** Ensure ADR-004 evolution rules support Phase 2 additions
- **Key items:** PeerHandshake, compression envelope field, multi-session messages

### Tuesday Afternoon (2026-08-18 14:00)

**Track A Kickoff:**
- networking-engineer: WebRTC integration spike (2-day exploration)
- rust-backend-lead: Multi-session architecture design
- principal-architect: Directory service prototype

**Track B Kickoff:**
- test-engineer-unit: Coverage measurement + headless GPU backend research
- devops-lead: Memory leak 5-min smoke test execution

---

## Risk Register Updates (§9.3)

### Risks Mitigated by Phase 1
- ✅ **Rust ramp-up** - Team velocity proven (multiple features delivered on time)
- ✅ **ConPTY integration** - 60 FPS rendering achieved
- ✅ **Monomind integration** - Detection + dashboard functional

### New Risks for Phase 2

**P2P WebRTC Integration (MEDIUM)**
- **Risk:** NAT traversal success rate in real-world networks
- **Mitigation:** STUN/TURN fallback architecture, connection quality metrics

**Multi-Session State Sync (MEDIUM)**
- **Risk:** Conflict resolution complexity in collaborative editing
- **Mitigation:** CRDTs or operational transforms, clear conflict policies

**SQLite Scale Limits (LOW → MEDIUM)**
- **Risk:** 1000 concurrent sessions may hit SQLite write contention
- **Mitigation:** WAL mode, connection pooling, consider sharding strategy

**Test Coverage Debt (MEDIUM)**
- **Risk:** 41% coverage insufficient for refactoring confidence
- **Mitigation:** Track B dedicated effort, headless GPU backend priority

**Memory Leak Validation (LOW)**
- **Risk:** AbortOnDrop fix may have edge cases
- **Mitigation:** Comprehensive soak test suite, memory profiling

---

## Success Metrics (Phase 2 → Phase 3 Gate)

Per SRS §7.2, Phase 2 completion requires:

1. **P2P connectivity** - 95% connection success rate (LAN + STUN/TURN)
2. **Multi-session management** - 10 concurrent sessions per master
3. **SQLite persistence** - Session state survives restart
4. **Multi-client collaboration** - 2+ clients attach to same session
5. **Test coverage** - ≥70% (Track B completion)
6. **24h soak test** - Zero crashes, <10% memory growth (Track B completion)

**Target Date:** Week 12 (6 weeks from Phase 2 start)

---

## Communication Protocol

**Daily Standups (Track A):**
- 09:30: networking-engineer (P2P progress)
- 09:45: rust-backend-lead (multi-session status)
- 10:00: principal-architect (architecture decisions)

**Weekly Gates (Track B):**
- Tuesday: Memory leak smoke test results (devops-lead)
- Friday: Coverage measurement + progress report (test-engineer-unit)

**Phase 2 → Phase 3 Gate Review:**
- Week 10: Preliminary assessment (eng-director + qa-lead)
- Week 11: Full gate criteria evaluation
- Week 12: Gate passage decision or contingency planning

---

## Appendix: Open Questions for Tuesday Architecture Session

1. **WebRTC Signaling:** Centralized signaling server vs DHT-based discovery?
2. **Directory Service:** Ephemeral (in-memory) vs persistent (SQLite)?
3. **Session State Sync:** CRDT (automerge) vs OT (operational transforms)?
4. **P2P Protocol Versioning:** How to handle version mismatch in Phase 2?
5. **Compression Strategy:** zstd on all messages or selective (OutputData only)?
6. **Authentication Model:** JWT extension for P2P or separate P2P auth tokens?

---

**Status:** DRAFT - Pending Criterion #5 fix completion  
**Activation:** If 5/7 achieved tonight → eng-director triggers Phase 2 Tuesday 09:00  
**Contingency:** If fix fails → Defer to Tuesday 09:00 debug war room, Phase 2 delayed

---

*This document prepared in advance to enable rapid Phase 2 kickoff if gate passage achieved Monday night. Do not distribute until trigger condition confirmed.*
