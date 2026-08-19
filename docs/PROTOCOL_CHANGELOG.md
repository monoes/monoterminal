# MONOTERMINAL Protocol Changelog

**Purpose:** Track all wire protocol changes across versions for backward/forward compatibility analysis.

**Policy:** Per ADR-004, every protocol buffer schema change MUST be documented here before merge.

**Version Scheme:**
- `v1.x` = Backward-compatible additions (new optional fields, new message types)
- `v2.x` = Breaking changes (field number changes, removals, package rename)

---

## v1.1 (Planned - Phase 1.5, Before Phase 2 P2P)

**Release Date:** TBD  
**Min Client:** 0.2.0  
**Min Server:** 0.2.0

### Added
- `protocol_version` field to `Envelope` (field 28, uint32)
  - Enables version negotiation between client/server
  - Defaults to 0 (v1.0 compatibility)
  - Required for Phase 2 P2P (mixed client versions)
- Version negotiation flow in AttachRequest/AttachResponse
- `ErrorCode::INCOMPATIBLE_VERSION` (value 7)

### Compatibility
- ✅ **Backward compatible** with v1.0 clients
- Old clients (v1.0) ignore `protocol_version` field (protobuf3 default)
- Old servers (v1.0) treat missing `protocol_version` as 0 (v1.0)
- New clients (v1.1) can downgrade to v1.0 when talking to old servers

### Migration Notes
- Clients SHOULD send `protocol_version=1` in AttachRequest
- Servers SHOULD respond with `protocol_version=min(client_version, server_version)`
- No action required for existing deployments (field is optional)

---

## v1.0 (Current - Phase 1 MVP)

**Release Date:** 2026-08-15 (initial implementation)  
**Min Client:** 0.1.0  
**Min Server:** 0.1.0

### Core Protocol

**Envelope** (top-level container):
- `sequence_number` (field 1, uint64) - Monotonic counter per connection
- `oneof message` (fields 2-17) - Message payload

**Session Management:**
- `AttachRequest` (field 2) - Join existing or create new session
  - `session_id` (string, UUID or empty for new)
  - `auth_token` (string, JWT)
  - `rows`, `cols` (uint32, terminal dimensions)
  - `last_seen_sequence` (uint64, for late-joiner sync, 0=full scrollback)
- `AttachResponse` (field 3) - Session metadata + scrollback
  - `session_id` (string, UUID)
  - `metadata` (SessionMetadata)
  - `scrollback` (repeated Line, last 10k lines)

**Terminal I/O:**
- `InputData` (field 4) - Client keyboard input
  - `data` (bytes, UTF-8)
  - `auth_token` (string, JWT)
- `OutputData` (field 5) - PTY output stream
  - `data` (bytes, PTY output chunk)
  - `sequence` (uint64, for ordering/dedup)
  - `compression` (CompressionType enum)
- `ResizeRequest` (field 6) - Change terminal dimensions
  - `rows`, `cols` (uint32)
  - `auth_token` (string, JWT)

**Connection Control:**
- `DetachRequest` (field 7) - Leave session (graceful disconnect)
  - `session_id` (string, UUID)
- `ErrorResponse` (field 8) - Server error notification
  - `code` (ErrorCode enum)
  - `message` (string, human-readable)

**Monomind Integration (SRS §2.4):**
- `DashboardRequest` (field 9) - Request monomind org/agent/run status
  - `command` (string, e.g., "status", "agents")
  - `params` (map<string,string>)
- `DashboardResponse` (field 10) - Monomind status data (structured fields, see ADR-004 §7)
  - `org_name` (string)
  - `org_status` (string, "running"|"stopped"|"error")
  - `agents` (repeated AgentInfo)
  - `tasks` (repeated TaskInfo)
  - `kg_stats` (KnowledgeGraphStats)
  - `timestamp` (int64, unix seconds)
- `HealthCheckRequest` (field 11) - Trigger monomind doctor check
  - `project_dir` (string, project root, defaults to session cwd)
- `HealthCheckResponse` (field 12) - Health check results
  - `installed` (bool)
  - `version` (string)
  - `control_server_reachable` (bool)
  - `broker_registered` (bool)
  - `last_check_timestamp` (int64, unix seconds)
  - `issues` (repeated HealthIssue)
- `UpgradeRequest` (field 13) - Trigger monomind upgrade
  - `project_dir` (string)
  - `confirmed` (bool, user confirmation required)
- `UpgradeResponse` (field 14) - Upgrade result
  - `success` (bool)
  - `old_version`, `new_version` (string)
  - `output` (string, command output)
- `DetectionRequest` (field 15) - Check for .monomind/ directory
  - `project_dir` (string)
- `DetectionResponse` (field 16) - Detection result
  - `found` (bool)
  - `monomind_root` (string, root dir containing .monomind/)
  - `suggest_install` (bool)
  - `dismiss_file_exists` (bool)
  - `banner_text` (string, MOTD-style)
- `MonitoringData` (field 17) - Live org/agent/run status stream
  - `org_name` (string)
  - `active_agents`, `running_tasks` (int32)
  - `kg_nodes`, `kg_relationships` (int64)
  - `kg_last_updated` (int64, unix seconds)
  - `recent_runs` (repeated RunSummary)

### Supporting Types

**SessionMetadata:**
- `shell_type` (string, e.g., "powershell.exe", "cmd.exe")
- `working_dir` (string, current working directory)
- `rows`, `cols` (uint32, current dimensions)
- `created_at`, `last_activity` (int64, unix seconds)

**Line** (scrollback entry):
- `data` (bytes, UTF-8 with ANSI codes)
- `line_number` (uint64, sequential)

**HealthIssue:**
- `severity` (IssueSeverity enum: INFO=0, WARNING=1, ERROR=2)
- `message` (string)
- `resolution` (string, suggested fix, optional)

**RunSummary:**
- `run_id`, `goal` (string)
- `started_at`, `completed_at` (int64, unix seconds, 0=still running)
- `status` (string, "running"/"completed"/"failed")

**AgentInfo** (monomind dashboard):
- `id`, `name`, `role` (string)
- `status` (string, "running"|"idle"|"stopped")
- `tasks_completed` (uint32)
- `uptime_secs` (uint64)

**TaskInfo** (monomind dashboard):
- `id`, `title`, `status`, `assignee` (string)
- `dependencies` (repeated string)

**KnowledgeGraphStats** (monomind dashboard):
- `nodes`, `relationships`, `total_entries`, `db_size_bytes` (uint64)
- `last_updated` (int64, unix seconds)

### Enums

**CompressionType:**
- `NONE = 0` (no compression)
- `ZSTD = 1` (zstd compression, future)

**ErrorCode:**
- `UNKNOWN = 0`
- `SESSION_NOT_FOUND = 1`
- `AUTH_FAILED = 2`
- `PERMISSION_DENIED = 3`
- `RATE_LIMIT_EXCEEDED = 4`
- `INVALID_REQUEST = 5`
- `SERVER_ERROR = 6`

**IssueSeverity:**
- `INFO = 0`
- `WARNING = 1`
- `ERROR = 2`

### Compatibility
- ✅ Initial release (no backward compatibility concerns)
- Field numbers 2-17 reserved for current features
- Field 28+ reserved for future extensions (per ADR-004)

---

## Version History Summary

| Version | Release Date | Breaking? | Key Features |
|---------|--------------|-----------|--------------|
| v1.0    | 2026-08-15   | N/A       | Initial protocol: session management, terminal I/O, monomind integration |
| v1.1    | TBD          | No        | Version negotiation (protocol_version field) |
| v2.0    | TBD          | Yes       | TBD (requires package rename to monoterminal.v2) |

---

## How to Update This Changelog

**When adding new fields/messages:**

```markdown
## vX.Y (Planned/Released - Phase Z)

**Release Date:** YYYY-MM-DD or TBD  
**Min Client:** X.Y.Z  
**Min Server:** X.Y.Z

### Added
- `new_field` to `ExistingMessage` (field N, type)
  - Purpose: What it does
  - Default: What old clients see
  - Required for: What feature needs it

### Compatibility
- ✅/❌ Backward compatible with vX.(Y-1)
- Migration notes for existing deployments

### Breaking Changes (if any)
- What changed and why
- Migration path for users
```

**References:**
- ADR-004: Protocol Schema Evolution Policy
- SRS §3.1.1: Protocol Buffers Schema
- Protobuf file: `proto/monoterminal/v1/messages.proto`

---

**Maintained by:** principal-architect  
**Last updated:** 2026-08-15  
**Next review:** Before Phase 2 P2P networking ships
