# Memory Leak Investigation - Monday 2026-08-17

**Owner:** rust-backend-lead  
**Priority:** P0  
**Status:** IN PROGRESS - Smoke test running

---

## Executive Summary

**Hypothesis:** Memory leak reported on Saturday 2026-08-16 00:27 AM was likely **already fixed** by the heap corruption fix committed 10 hours later at 11:10 AM (commit 8d3259c).

**Current Action:** Running 1-hour smoke test to verify fix.

---

## Timeline Analysis

| Event | Date/Time | Details |
|-------|-----------|---------|
| Smoke Test Failed | Sat Aug 16 00:27 AM | 52.1% WS growth, 244% PM growth, 46.3% handle growth in 5 min |
| Heap Corruption Fix | Sat Aug 16 11:10 AM | Commit 8d3259c - Safe handle management in windows.rs |
| Investigation Start | Mon Aug 17 ~02:00 | rust-backend-lead analysis |
| Current Build Test | Mon Aug 17 02:30 | 0.8% WS growth over 3.5min - STABLE |
| Smoke Test Launch | Mon Aug 17 02:35 | Official 1-hour smoke test started (PID 16308) |

**Critical Gap:** 10+ hours between test failure and fix commit.

---

## Evidence: Leak Was Already Fixed

### 1. Timeline Mismatch
- Failed smoke test ran on code **BEFORE** commit 8d3259c
- Current HEAD includes the heap corruption fix
- Current build shows NO leak (0.8% growth vs 52.1% previously)

### 2. Symptoms Align with Handle Management Issues
**Original leak symptoms:**
- Working Set: +52.1% (memory pressure)
- Private Bytes: +244% (heap allocations)
- **Handle Count: +46.3%** ← Key indicator

**Heap corruption fix addressed:**
- Unsafe `mem::forget` patterns in handle ownership transfer
- Added `Handle::into_raw()` for safe ownership transfer
- Proper RAII cleanup in Drop implementations

**Analysis:** Handle leaks cause both memory growth (kernel objects) and handle count growth. The 46.3% handle growth strongly suggests handle management was the root cause.

### 3. Current Build Validation (3.5 min idle monitoring)

```
Baseline:  WS: 16.16 MB | PM: 21.3 MB  | Handles: 187
t=1m30s:   WS: 16.33 MB | PM: 21.26 MB | Handles: 187 (1.0% growth)
t=2m0s:    WS: 16.29 MB | PM: 21.2 MB  | Handles: 187 (0.8% growth)
t=3m30s:   WS: 16.29 MB | PM: 21.2 MB  | Handles: 187 (0.8% growth)
```

**Verdict:** Completely stable - no leak detected.

---

## What the Heap Corruption Fix Changed

**Commit:** 8d3259c86cf62f767f5b11bb01114ab15388cdd3  
**Author:** nokhodian  
**Date:** Sun Aug 16 11:10:25 2026 +0200

### Changes
1. **Created `windows.rs` module** (569 lines)
   - Factored out Windows-specific handle management
   - Added safe ownership transfer patterns

2. **Key Fix: `Handle::into_raw()` method**
   ```rust
   /// Transfer ownership of the raw handle, preventing Drop from closing it
   ///
   /// # Safety
   /// The caller must ensure the returned HANDLE is eventually closed,
   /// either manually or by transferring ownership to another wrapper.
   fn into_raw(self) -> HANDLE {
       let handle = self.0;
       std::mem::forget(self); // Prevent Drop from closing the handle
       handle
   }
   ```

3. **Eliminated unsafe `mem::forget` patterns**
   - Before: Direct `mem::forget` calls in spawn() method
   - After: Encapsulated in `Handle::into_raw()`
   - Impact: Prevents undefined behavior in handle ownership transfer

---

## Alternative Leak Suspects (If Smoke Test Fails)

If the 1-hour smoke test fails, investigate these in order:

### 1. RateLimiter HashMap Accumulation (HIGH PROBABILITY)

**File:** `crates/master/src/auth/rate_limit.rs`

**Issue:** Three HashMaps accumulate entries without cleanup:
- `connection_buckets: HashMap<SocketAddr, TokenBucket>`
- `auth_trackers: HashMap<SocketAddr, AuthFailureTracker>`
- `session_buckets: HashMap<String, TokenBucket>`

**Root Cause:** `cleanup()` method exists (line 204) but is **NEVER CALLED**:
```rust
/// Cleanup expired entries (call periodically)
pub fn cleanup(&self) {
    let now = Instant::now();
    
    // Cleanup auth trackers with expired bans
    let mut trackers = self.auth_trackers.lock().unwrap();
    trackers.retain(|_, tracker| {
        !tracker.failures.is_empty() || tracker.is_banned()
    });
    
    // Note: buckets cleanup on access (lazy cleanup via refill)
}
```

**Evidence:** Grep for `rate_limiter.cleanup()` returns NO results.

**Fix:** Add periodic cleanup task in main.rs:
```rust
// 7. Start RateLimiter cleanup task (every 5 minutes)
tokio::spawn({
    let rate_limiter = Arc::clone(&rate_limiter);
    async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300));
        loop {
            interval.tick().await;
            rate_limiter.cleanup();
        }
    }
});
```

### 2. Tracing Infrastructure (JSON Logging)

**File:** `crates/master/src/main.rs` lines 46-59

**Issue:** SOAK_TEST_MODE enables JSON logging to hourly-rotated files with non-blocking writer:
```rust
let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
Box::leak(Box::new(_guard)); // Guard leaked intentionally
```

**Potential Leak:** Non-blocking writer might accumulate buffered messages.

**Investigation:** Check if tracing buffer grows unbounded.

### 3. Broadcast Channel (Health Status)

**File:** `crates/master/src/main.rs` line 110

**Issue:** Broadcast channel for health status updates:
```rust
let (health_tx, _health_rx) = broadcast::channel::<HealthStatus>(16);
```

**Analysis:** Broadcast channels don't accumulate - they drop old messages if no receivers. **Low probability** of leak here.

---

## Smoke Test Specification

**Test:** `scripts/memory_smoke_test_clean.ps1`  
**Duration:** 60 minutes  
**Sample Interval:** 5 minutes  
**Samples:** 12 total

### Pass Criteria
- **WS growth:** <1% (strict threshold for smoke test)
- **PM growth:** <5%
- **Handle delta:** ≤5
- **No crashes**

### Expected Results (If Fix is Confirmed)
Based on 3.5min validation showing 0.8% growth, expect:
- WS growth: ~0.5-2% (well below 1% threshold)
- PM growth: ~0-1% (well below 5% threshold)
- Handle delta: 0-2 (well below 5 threshold)

---

## Next Steps

### If Smoke Test PASSES
1. ✅ Declare memory leak FIXED by commit 8d3259c
2. Document fix in commit message / ADR
3. Coordinate with devops-lead for Wed-Thu 24h soak test
4. Update VERIFICATION.md with PASS status
5. Report to eng-director: 7/7 timeline back on track

### If Smoke Test FAILS
1. Review failure metrics vs original smoke test
2. Investigate RateLimiter cleanup (highest probability)
3. Profile with Windows Performance Recorder
4. Implement fix + re-test
5. Escalate to principal-architect if deep architectural issue

---

## Coordination

**Stakeholders:**
- eng-director: Overall timeline coordination
- devops-lead: 24h soak test execution (if smoke passes)
- qa-lead: Verification wave coordination
- performance-engineer: 24h monitoring support

**Delivery Target:** Monday 2 PM (smoke test results + analysis)

---

**Status:** ⏳ WAITING - Smoke test running (started 02:35, expected completion 03:35)
