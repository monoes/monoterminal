# D8.3.Q5 cmux Performance Benchmarks - Research Report

**Agent:** researcher  
**Date:** 2026-08-14  
**Task:** D8.3.Q5 - cmux performance benchmarks  
**Status:** ❌ INCOMPLETE - Data Unavailable

---

## Critical Finding: Benchmarks Do Not Exist in Accessible Sources

### 🔴 Blockers

1. **cmux launched Feb 2026** (POST training cutoff Jan 2025)
   - Zero cmux-specific data in training corpus
   - All cmux knowledge post-dates training window

2. **WebSearch/WebFetch tools BLOCKED**
   - Cannot access GitHub issues at manaflow-ai/cmux
   - Cannot search web for performance reports
   - Cannot verify any benchmark claims

3. **Architectural Mismatch**
   - cmux is NOT a traditional multiplexer (like tmux/screen)
   - It's a macOS GUI app (Swift + Ghostty wrapper)
   - Standard metrics (sessions/instance, client connections) **DO NOT APPLY**

---

## What We Know About cmux Architecture

From D8.3 findings (Q1-Q4):

**Architecture:**
- macOS-only GUI application (Swift UI + Ghostty core)
- Each vertical tab = separate Ghostty instance
- Unix domain socket IPC between tabs
- ~5k lines of code (lightweight)

**NOT a Client-Server System:**
- No network protocol
- No remote clients
- Single-user, single-machine only
- Cannot compare to tmux's session/client model

**Performance Implications:**
- Limited by macOS process/memory limits
- GPU rendering (Metal) per tab - memory intensive
- Swift UI overhead vs terminal-only apps

---

## Research Question Analysis

### Original Question
> "cmux performance benchmarks: sessions per instance, memory usage per session, client connection limits, network bandwidth per client"

### Question Validity Assessment

| Metric | Applicability to cmux | Reason |
|--------|----------------------|---------|
| Sessions per instance | ❌ NOT APPLICABLE | cmux uses "tabs", not "sessions"; different concept |
| Memory per session | ⚠️ REFRAME NEEDED | Should be "memory per tab" instead |
| Client connections | ❌ NOT APPLICABLE | No client-server architecture exists |
| Bandwidth per client | ❌ NOT APPLICABLE | No network protocol exists |

**Correct Question for cmux:**
> "cmux resource usage: tabs per instance, memory usage per tab, tab creation latency, macOS resource limits"

---

## Comparison Baseline: tmux (Reference Only)

Since cmux benchmarks unavailable, tmux metrics provided for context:

**tmux Performance (from training data):**
- **Sessions per instance:** 100-500 typical, 1000+ in stress tests
- **Memory per session:** Base 10-50MB + 2-5MB per session
- **Client connections:** Multiple clients can attach to same session
- **Bandwidth:** 1-10 Kbps typical text, bursts to 100 Kbps

**Data Quality:** RECONSTRUCTED_FROM_TRAINING  
**Relevance:** LOW - tmux is client-server, cmux is GUI app

---

## Ghostty Rendering Impact

cmux wraps Ghostty, so Ghostty performance affects cmux:

**Ghostty Characteristics:**
- GPU-accelerated (Metal on macOS)
- High FPS (60+ FPS) smooth scrolling
- Memory-intensive (GPU textures, glyph cache)
- Estimated 50-200MB per instance (includes GPU buffers)

**Implication for cmux:**
- Each cmux tab = full Ghostty instance
- Higher memory per tab than tmux sessions
- Better rendering performance (GPU)
- macOS memory limits become bottleneck faster

---

## Gap Assessment

**Question Answerable:** ❌ NO  
**Minimum Sources (2+):** ❌ NO - Zero authoritative sources  
**Quantitative Metrics:** ❌ NO - No numbers with units  
**Cross-Verification:** ❌ IMPOSSIBLE - No sources to verify  
**Actionable Depth:** ❌ NO - Cannot make architectural decisions

**Quality Standard:** FAILED - Does not meet AGGRESSIVE TECHNICAL AUDITOR MODE requirements

---

## Recommended Next Steps

### Option 1: Approve Web Tools (RECOMMENDED)

**Required Approvals:**
- WebSearch
- WebFetch

**Search Strategy:**
```
GitHub: repo:manaflow-ai/cmux (performance OR benchmark OR memory OR scalability)
Web: "cmux performance benchmarks"
Web: "cmux vs tmux resource usage"
Issues: manaflow-ai/cmux labels:performance,bug
Discussions: manaflow-ai/cmux search:memory,slow,lag
```

**Expected Outcome:** User reports, GitHub issue discussions, possibly official benchmarks

---

### Option 2: Manual Testing

**Setup:**
1. Install cmux on macOS
2. Install Activity Monitor or `htop`
3. Baseline: Measure single Ghostty instance

**Test Protocol:**
1. Open 1 tab: Record memory (private + shared + GPU)
2. Open 10 tabs: Record memory, CPU, latency
3. Open 50 tabs: Record performance degradation
4. Open 100 tabs: Find breaking point
5. Measure tab creation/close latency

**Deliverables:** Real quantitative data with units (MB, %, ms)

---

### Option 3: Mark as Unavailable

**Action:** Document in knowledge matrix:
```json
{
  "question": "cmux performance benchmarks",
  "answer": "UNAVAILABLE - cmux launched Feb 2026 (post-training), no public benchmarks found, web tools blocked. Architectural analysis: cmux is macOS GUI (not multiplexer), traditional metrics not applicable. Recommend manual testing or web research when tools approved.",
  "sources": ["https://github.com/manaflow-ai/cmux"],
  "data_quality": "UNAVAILABLE",
  "metrics": {
    "sessions_per_instance": "N/A - GUI app uses tabs",
    "memory_per_session_mb": "UNKNOWN - benchmarks needed",
    "client_connections_limit": "N/A - no client-server",
    "bandwidth_per_client_mbps": "N/A - no network protocol"
  }
}
```

---

## Conclusion

**Status:** D8.3.Q5 CANNOT BE COMPLETED with current limitations.

**Root Cause:**
1. cmux is too new (Feb 2026, post-training)
2. Web access blocked (cannot research GitHub/docs)
3. No alternative data sources exist

**Impact on Knowledge Matrix:**
- D8.3 cannot reach 85%+ target without this finding
- D8 domain completeness blocked at current level
- Architectural decisions for MONOTERMINAL lack cmux performance comparison

**Recommendation:** Prioritize WebSearch/WebFetch approval OR manual testing to unblock D8 completion.

---

## Files Generated

1. `/Volumes/SD1/projects/monomux/research-d83q5-cmux-performance.json` - Full findings
2. `/Volumes/SD1/projects/monomux/research-d83q5-summary.md` - This summary

---

## Coordinator Handoff

**Status:** ❌ BLOCKED - Data unavailable  
**Blocker:** WebSearch/WebFetch approval OR manual testing required  
**Next Agent:** Coordinator to decide: approve web tools, assign manual testing, or proceed without data  
**Deliverable:** Knowledge Matrix D8.3 CANNOT be updated without valid findings
