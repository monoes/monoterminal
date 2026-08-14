# MONOTERMINAL — Handoff Package

**Start here:** `monoterminal-srs.md`

That single file is the complete, implementation-ready Software Requirements Specification (v1.2). It is self-contained — §9.5 in that document ("Implementation Quick-Start") has everything needed to begin Sprint 0 on a fresh Windows machine: toolchain prerequisites, a suggested repo layout, a starter `Cargo.toml`, and first-day commands. You do not need anything else in this package to start building.

## What else is in here

- **`knowledge-matrix-monoterminal.json`** — the full research base the SRS was synthesized from (95% complete, 46/46 knowledge-matrix nodes). Every `[Dx.y]` reference in the SRS traces back to a node in here.
- **`research/`** — the underlying per-domain research: raw findings (`research-*.json`), human-readable summaries (`research-*-summary.md`, `D4-STREAMING-LATENCY-SUMMARY.md`, `D6_RESEARCH_SUMMARY.md`), an earlier architecture draft (`MONOTERMINAL-ASD.md`), and process artifacts from the research batches (`batch3-templates.json`, `batch4-completion-report.md`, `d4-completion-report.md`).

These are only useful for auditing *why* a specific decision was made, beyond what's already written out in the SRS's own Decision Log (§8). Nothing in `research/` is required to implement the product — only `monoterminal-srs.md` is.

## Version

SRS v1.2 (2026-08-14) — Windows-first rollout (Phase 1), web-only PWA client, monomind integration first-class. Full revision history is in the SRS's own "Document Control" table at the top of the file.
