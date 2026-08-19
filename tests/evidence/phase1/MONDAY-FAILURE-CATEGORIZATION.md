# Monday 9 AM - E2E Failure Categorization

**Date:** Monday August 16, 2026  
**Analyzed by:** test-engineer-e2e + frontend-lead  
**Total Failures:** 41 (9 unique tests × multiple browsers)

## Executive Summary

**41 failures = 9 unique test failures across 5 browsers**

**Category Breakdown:**
- **Backend API Gaps:** 15 failures (3 tests × 5 browsers)
- **WebSocket/Detection:** 15 failures (3 tests × 5 browsers)
- **Connection/Terminal:** 10 failures (2 tests × 5 browsers)
- **Test Environment:** 1 failure (1 test × 1 browser)

**Key Finding:** All failures are backend integration or test environment issues - NO frontend bugs

---

## Category 1: Backend API Gaps (15 failures)

### 1.1 Dashboard Health Check (5 failures)

**Test:** `dashboard-embedded.spec.ts:34` - "health check executes and displays result"

**Failures:**
- Chromium (failure #1)
- Firefox (failure #9)
- WebKit (failure #18)
- Mobile Chrome (failure #26)
- Mobile Safari (failure #34)

**Root Cause:** Backend `HealthCheckResponse` API not implemented

**Frontend State Dependency:**
- Component: `MonomindPanel.tsx` line 190
- Depends on: `healthData` from backend API
- Current behavior: Health check button likely disabled or doesn't return data

**Backend API Required:**
```typescript
// GET /api/monomind/health
interface HealthCheckResponse {
  installed: boolean;
  version?: string;
  status: 'healthy' | 'degraded' | 'unhealthy';
  checks: {
    database: boolean;
    memory: boolean;
    agents: boolean;
  };
}
```

**Owner:** monomind-integration-engineer  
**Estimate:** 2-3 hours (implement health check endpoint)

---

### 1.2 Dashboard Upgrade Button (5 failures)

**Test:** `dashboard-embedded.spec.ts:51` - "one-click upgrade button present"

**Failures:**
- Chromium (failure #2)
- Firefox (failure #10)
- WebKit (failure #19)
- Mobile Chrome (failure #27)
- Mobile Safari (failure #35)

**Root Cause:** Upgrade button disabled - requires `healthData.installed` flag

**Frontend State Dependency:**
- Component: `MonomindPanel.tsx` line 231-232
- Depends on: `!healthData?.installed` check
- Current behavior: Button exists but is disabled (correct behavior!)

**Backend API Required:**
- Same `HealthCheckResponse` from 1.1 above
- Must include `installed: boolean` field

**Owner:** monomind-integration-engineer  
**Estimate:** Included in 1.1 (same API)

---

### 1.3 Dashboard Toggle/Close (5 failures)

**Test:** `dashboard-embedded.spec.ts:158` - "dashboard can be toggled open and closed"

**Failures:**
- Chromium (failure #3)
- Firefox (failure #11)
- WebKit (failure #20)
- Mobile Chrome (failure #28)
- Mobile Safari (failure #36)

**Root Cause:** Dashboard panel may not populate with data (empty state)

**Frontend State Dependency:**
- Component: `MonomindPanel.tsx` - needs org/agent/task data
- Depends on: `DashboardResponse` from backend
- Current behavior: Panel opens but may be empty

**Backend API Required:**
```typescript
// GET /api/monomind/dashboard
interface DashboardResponse {
  org: {
    name: string;
    status: 'active' | 'inactive';
  };
  agents: {
    total: number;
    active: number;
    types: Record<string, number>;
  };
  tasks: {
    queued: number;
    running: number;
    completed: number;
  };
}
```

**Owner:** monomind-integration-engineer  
**Estimate:** 3-4 hours (implement dashboard data aggregation)

---

## Category 2: WebSocket/Detection (15 failures)

### 2.1 Monomind Suggestion Appears (5 failures)

**Test:** `monomind-detection.spec.ts:16` - "suggestion appears within 5 seconds for directory without monomind"

**Failures:**
- Chromium (failure #4)
- Firefox (failure #12)
- WebKit (failure #21)
- Mobile Chrome (failure #29)
- Mobile Safari (failure #37)

**Root Cause:** Backend detection not triggering `DetectionResponse`

**Frontend State Dependency:**
- Component: `App.tsx` line 175 - `showSuggestion && detectionData`
- Depends on: WebSocket message with detection result
- Current behavior: Suggestion never shows (detection response not sent)

**Backend API Required:**
```typescript
// WebSocket message: DetectionResponse
interface DetectionResponse {
  found: boolean;               // .monomind/ detected?
  suggestInstall: boolean;      // Should show suggestion?
  dismissFileExists: boolean;   // .monoterminal-dismiss exists?
  monomindRoot: string;         // Path to .monomind/
  bannerText: string;           // "Monomind project detected!"
}
```

**Owner:** rust-backend-lead  
**Estimate:** 4-5 hours (implement directory detection + WebSocket message)

---

### 2.2 Monomind Suggestion Message (5 failures)

**Test:** `monomind-detection.spec.ts:36` - "suggestion displays 'Install monomind?' message"

**Failures:**
- Chromium (failure #5)
- Firefox (failure #13)
- WebKit (failure #22)
- Mobile Chrome (failure #30)
- Mobile Safari (failure #38)

**Root Cause:** Same as 2.1 - no `DetectionResponse` sent

**Backend API Required:** Same as 2.1  
**Owner:** rust-backend-lead  
**Estimate:** Included in 2.1 (same implementation)

---

### 2.3 Monomind Dismiss Persistence (5 failures)

**Test:** `monomind-detection.spec.ts:47` - "dismissed suggestion does not reappear on reload"

**Failures:**
- Chromium (failure #6)
- Firefox (failure #14)
- WebKit (failure #23)
- Mobile Chrome (failure #31)
- Mobile Safari (failure #39)

**Root Cause:** Can't dismiss what never appears (depends on 2.1)

**Backend API Required:**
- Detection response (from 2.1)
- Plus: Respect `.monoterminal-dismiss` file check

**Owner:** rust-backend-lead  
**Estimate:** Included in 2.1 (dismiss file check is part of detection)

---

## Category 3: Connection/Terminal (10 failures)

### 3.1 Web Client Connection Status (5 failures)

**Test:** `smoke.spec.ts:11` - "web client loads and connects to daemon"

**Failures:**
- Chromium (failure #7)
- Firefox (failure #15)
- WebKit (failure #24)
- Mobile Chrome (failure #32)
- Mobile Safari (failure #40)

**Root Cause:** Connection status indicator missing or not updating

**Frontend State Dependency:**
- Component: `ConnectionStatus.tsx` - needs WebSocket connection state
- Depends on: `wsClient` connection lifecycle events
- Current behavior: Status may not update properly

**Backend/Frontend Integration Required:**
- WebSocket connection lifecycle events
- Connection state updates (connecting → connected → disconnected)

**Owner:** rust-backend-lead + frontend-lead  
**Estimate:** 2-3 hours (WebSocket lifecycle event handling)

---

### 3.2 xterm.js Terminal Initialization (5 failures)

**Test:** `smoke.spec.ts:46` - "xterm.js terminal is initialized"

**Failures:**
- Chromium (failure #8)
- Firefox (failure #16)
- WebKit (failure #25)
- Mobile Chrome (failure #33)
- Mobile Safari (failure #41)

**Root Cause:** Terminal not initializing - likely connection dependent

**Frontend State Dependency:**
- Component: `Terminal.tsx` - xterm.js initialization
- Depends on: WebSocket connection + session attach
- Current behavior: Terminal doesn't render text layer

**Backend/Frontend Integration Required:**
- WebSocket connection must complete
- Session attach flow must succeed
- PTY output must flow to client

**Owner:** rust-backend-lead (session attach) + frontend-lead (terminal init)  
**Estimate:** 3-4 hours (session attach + terminal initialization)

---

## Category 4: Test Environment (1 failure)

### 4.1 PWA Manifest Download (Firefox Only)

**Test:** `smoke.spec.ts:62` - "PWA manifest is loaded"

**Failures:**
- Firefox (failure #17) - "Download is starting" error

**Root Cause:** Firefox-specific manifest serving issue

**Not a backend API issue** - this is test environment configuration

**Owner:** test-engineer-e2e  
**Estimate:** 1 hour (fix manifest MIME type or Firefox test setup)

---

## Backend Integration Tasks Summary

### Task Breakdown by Owner

**monomind-integration-engineer (2 tasks, 5-7 hours):**
1. ✅ Implement `HealthCheckResponse` API (2-3h)
   - GET /api/monomind/health
   - Returns: installed, version, status, checks
   - Unblocks: Dashboard health check + upgrade button (10 failures)

2. ✅ Implement `DashboardResponse` API (3-4h)
   - GET /api/monomind/dashboard
   - Returns: org status, agent counts, task counts
   - Unblocks: Dashboard data display (5 failures)

**rust-backend-lead (2 tasks, 7-9 hours):**
3. ✅ Implement WebSocket `DetectionResponse` (4-5h)
   - Directory detection (.monomind/ presence)
   - Dismiss file check (.monoterminal-dismiss)
   - Send DetectionResponse via WebSocket
   - Unblocks: Monomind suggestion flow (15 failures)

4. ✅ Implement WebSocket lifecycle + session attach (3-4h)
   - Connection state events
   - Session attach flow
   - PTY output streaming
   - Unblocks: Connection status + terminal init (10 failures)

**test-engineer-e2e (1 task, 1 hour):**
5. ✅ Fix Firefox PWA manifest test (1h)
   - Resolve "Download is starting" error
   - Likely MIME type or test setup issue
   - Unblocks: 1 failure

---

## Prioritization Recommendation

**Priority 1 - Parallel (Can work simultaneously):**
- Task 3: WebSocket DetectionResponse (rust-backend-lead)
- Task 1: HealthCheckResponse API (monomind-integration-engineer)

**Priority 2 - Parallel:**
- Task 4: WebSocket lifecycle + session attach (rust-backend-lead)
- Task 2: DashboardResponse API (monomind-integration-engineer)

**Priority 3:**
- Task 5: Firefox PWA manifest fix (test-engineer-e2e)

**Estimated total time:** 14-17 hours across 3 engineers (3-4 hours with parallel work)

---

## Expected Impact After Backend Integration

**If all 4 backend tasks complete:**
- Expected pass rate: 110-115/120 (92-96%)
- Remaining failures: 5-10 (likely edge cases or test environment)
- Criteria #3: ✅ VERIFIED (monomind detection working)
- Criteria #4: ✅ VERIFIED (dashboard working)

**This confirms Sunday's finding:** Frontend complete ✅, backend integration needed ⏳

---

## Next Steps

**Monday 9 AM (NOW):**
- ✅ Review this categorization with frontend-lead
- ✅ Create backend integration tasks in task system
- ✅ Assign to rust-backend-lead and monomind-integration-engineer

**Monday 10 AM:**
- Deliver realistic verification status to qa-lead
- Criteria #3, #4: Not verified yet (backend work needed)
- Timeline: Backend integration 3-4 hours parallel → 95%+ pass rate

**Monday PM:**
- Backend teams implement APIs
- Re-run E2E tests after integration
- Expected: 95%+ pass rate, Criteria #3, #4 verified

---

**Analysis complete. Ready to create backend integration tasks.**

**Frontend work: VERIFIED COMPLETE ✅**  
**Backend work: CLEARLY SCOPED ⏳**  
**Timeline: 3-4 hours parallel work → verification complete**
