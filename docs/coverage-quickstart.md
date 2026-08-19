# Coverage Measurement Quick Start

**Purpose:** Establish Phase 1 baseline coverage for SRS §7.1 criterion #6 verification.

---

## Prerequisites

### 1. Install Rust (if not already installed)

**Windows:**
```powershell
# Download and run rustup-init.exe from https://rustup.rs
# Or use winget:
winget install Rustlang.Rustup

# Verify installation
cargo --version
rustc --version
```

**Expected output:**
```
cargo 1.XX.X
rustc 1.XX.X
```

### 2. Install cargo-tarpaulin

```powershell
cargo install cargo-tarpaulin
```

⏱️ **Note:** This takes ~10 minutes to compile. Get coffee.

---

## Running Coverage

### Quick Run (recommended)

Configuration is already in `.tarpaulin.toml`, so just run:

```powershell
cargo tarpaulin --workspace --all-features --out Html --out Xml --out Json --timeout 300
```

⏱️ **Estimated time:** 5-15 minutes depending on test suite size

### What It Does

1. **Compiles** all workspace crates with instrumentation
2. **Runs** all unit, integration, and doc tests
3. **Generates** coverage reports in multiple formats:
   - `coverage/index.html` - Interactive HTML report (open in browser)
   - `coverage/cobertura.xml` - XML for CI/Codecov
   - `tarpaulin-report.json` - JSON with detailed metrics

### Excluded from Coverage

Per `.tarpaulin.toml`:
- ✅ Generated protobuf code (`crates/protocol/src/generated/*`)
- ✅ Test files (`*/tests/*`)
- ✅ Benchmarks (`*/benches/*`)
- ✅ Build scripts (`*/build.rs`)

---

## Reading Results

### Quick Check: JSON

```powershell
# Extract total coverage percentage
Get-Content tarpaulin-report.json | ConvertFrom-Json | Select-Object -ExpandProperty coverage
```

**Expected:** `XX.XX` (percentage as decimal, e.g., 72.5 = 72.5%)

### Detailed View: HTML

```powershell
# Open HTML report in browser
Start-Process coverage/index.html
```

**Look for:**
- Total workspace coverage (top of page)
- Per-crate breakdown (expandable sections)
- Per-file coverage (drill down for details)
- Highlighted uncovered lines (red = not covered)

### CI-Compatible: XML

```powershell
# Codecov expects this file
coverage/cobertura.xml
```

---

## Interpreting Results

### ✅ Success: ≥70% Coverage

**Action:** Report to qa-lead that criterion #6 is VERIFIED.

**Evidence needed:**
- Total percentage
- Per-crate breakdown
- Screenshots of HTML report (optional)

### ❌ Gap: <70% Coverage

**Action:** Calculate gap and identify priority modules.

**Example:**
- Current: 65%
- Target: 70%
- Gap: 5% = ~XXX lines to cover
- Priority: Modules with <60% coverage

---

## Troubleshooting

### Build Errors

```powershell
# Ensure everything compiles first
cargo build --workspace --all-features

# Check for test failures
cargo test --workspace --all-features
```

### Timeout Issues

If coverage times out (>300s), increase timeout in `.tarpaulin.toml`:

```toml
[run]
timeout = "600s"  # Increase from 300s
```

### Missing Coverage

If coverage shows 0% or unexpectedly low:

1. **Check test count:**
   ```powershell
   cargo test --workspace -- --list | Measure-Object -Line
   ```
   Expected: ~195 tests (per qa-lead's assessment)

2. **Verify LLVM engine:**
   ```powershell
   # Should use LLVM (more accurate than default)
   cargo tarpaulin --engine llvm --workspace
   ```

---

## Delivering Results to qa-lead

Once you have results, test-engineer-unit will:

1. Populate `docs/coverage-baseline-report.md`
2. Archive evidence files (`coverage/`, `tarpaulin-report.json`)
3. Send report via org_send to qa-lead
4. Update Phase 1 criterion #6 status

**Timeline:** ~30 minutes from data to delivered report.

---

## Reference

- **SRS Target:** §7.1 criterion #6 - ≥70% unit test coverage
- **Config:** `.tarpaulin.toml` (LLVM engine, 70% min-coverage threshold)
- **CI Workflow:** `.github/workflows/test.yml` (coverage job)
- **Codecov:** https://codecov.io/gh/monoterminal/monoterminal
