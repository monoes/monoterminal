# MONOTERMINAL Web Client Type Definitions

This directory contains TypeScript type definitions that correspond 1:1 with the Protocol Buffers schema defined in `proto/monoterminal/v1/messages.proto`.

## File Organization

```
types/
├── index.ts          # Barrel export - import all types from here
├── protocol.ts       # Core wire protocol types
├── health.ts         # Health check & upgrade types
├── dashboard.ts      # Dashboard, detection & monitoring types
└── README.md         # This file
```

## Type Categories

### 1. Core Protocol (`protocol.ts`)

Wire protocol types for terminal communication (SRS §3.1.1):

- `Envelope` - Top-level message wrapper with sequence number
- `AttachRequest/Response` - Session attachment and scrollback sync
- `InputData/OutputData` - Bidirectional terminal I/O
- `ResizeRequest` - Terminal dimension changes
- `ErrorResponse` - Server error reporting
- `SessionMetadata` - Session state and metadata

**Enums:**
- `CompressionType` - NONE, ZSTD
- `ErrorCode` - Error classification

### 2. Health Check & Upgrade (`health.ts`)

Monomind health monitoring and upgrade flow (SRS §2.4.3):

- `HealthCheckRequest/Response` - Health status queries
- `UpgradeRequest/Response` - One-click upgrade flow
- `HealthIssue` - Individual health check findings
- `IssueSeverity` - INFO, WARNING, ERROR

**Helper Functions:**
- `computeHealthStatus()` - Aggregate health from response
- `formatTimestamp()` - Display-friendly timestamps
- `getStatusEmoji/Text()` - UI rendering helpers

### 3. Dashboard & Monitoring (`dashboard.ts`)

Embedded dashboard data and detection (SRS §2.4.1, §2.4.2):

#### Request/Response Types
- `DashboardRequest/Response` - Generic dashboard command/response
- `DetectionRequest/Response` - Per-session `.monomind/` detection

#### Backend JSON Schema Types
These match the Rust `dashboard.rs` implementation:
- `DashboardData` - Top-level aggregate structure
- `OrgStatus` - Organization runtime state
- `AgentInfo` - Individual agent details
- `RunInfo` - Run history entry
- `MemoryStats` - Memory and knowledge graph statistics

#### Protobuf Streaming Types
- `MonitoringData` - Live org/agent/knowledge-graph stats (protobuf stream)
- `RunSummary` - Run history entries (protobuf format)

**Helper Functions:**
- `parseDashboardResponse<T>()` - Generic safe JSON parsing
- `parseDashboardData()` - Typed parser for DashboardData
- `formatRunDuration()` - Human-readable durations
- `formatRelativeTime()` - "5m ago" formatting
- `formatDbSize()` - Format bytes to KB/MB/GB
- `formatUptime()` - Format seconds to human-readable uptime
- `getRunStatusClass/Text()` - CSS class and display text for runs
- `getAgentStatusClass()` - CSS class for agent status

## Usage Examples

### Import Types

```typescript
// Import specific types
import { ErrorCode, HealthCheckRequest } from '@/types';

// Import everything
import * as Types from '@/types';
```

### Type Safety with Protocol Messages

```typescript
import { AttachRequest, ErrorCode } from '@/types';

const request: AttachRequest = {
  sessionId: '',
  authToken: jwt,
  rows: 24,
  cols: 80,
  lastSeenSequence: 0,
};

// TypeScript will enforce all required fields
```

### Health Check Example

```typescript
import {
  HealthCheckResponse,
  computeHealthStatus,
  getStatusEmoji,
} from '@/types';

function displayHealth(response: HealthCheckResponse) {
  const status = computeHealthStatus(response);
  const emoji = getStatusEmoji(status);

  console.log(`${emoji} ${status}`);
  console.log(`Version: ${response.version}`);
  console.log(`Issues: ${response.issues.length}`);
}
```

### Dashboard Command Example

```typescript
import {
  DashboardRequest,
  DashboardData,
  parseDashboardData,
  formatDbSize,
  formatUptime,
  getAgentStatusClass,
} from '@/types';

// Send dashboard request
const request: DashboardRequest = {
  command: 'status',
  params: {},
};

// Later, when response arrives - use typed parser
const data = parseDashboardData(response);
if (data) {
  // Access org status
  console.log(`Org running: ${data.org_status.running}`);
  console.log(`Active agents: ${data.org_status.active_agents}`);

  // Render agent list
  data.agents.forEach(agent => {
    const statusClass = getAgentStatusClass(agent.status);
    const uptime = formatUptime(agent.uptime_secs);
    console.log(`${agent.agent_type}: ${agent.status} (${uptime})`);
  });

  // Display memory stats
  const dbSize = formatDbSize(data.memory_stats.db_size_bytes);
  console.log(`Database: ${dbSize} (${data.memory_stats.kg_nodes} nodes)`);

  // Show run history
  data.runs.forEach(run => {
    console.log(`Run ${run.id}: ${run.outcome} (${run.tokens} tokens)`);
  });
}
```

## Naming Conventions

### Proto ↔ TypeScript Mapping

| Protobuf Convention | TypeScript Convention | Example |
|---------------------|----------------------|---------|
| `snake_case` fields | `camelCase` properties | `auth_token` → `authToken` |
| `PascalCase` messages | `PascalCase` interfaces | `HealthCheckRequest` → `HealthCheckRequest` |
| `SCREAMING_SNAKE_CASE` enums | `PascalCase` enum values | `RATE_LIMIT_EXCEEDED` → `RATE_LIMIT_EXCEEDED` |

### Type vs Interface

- **Interface** for data structures (e.g., `AttachRequest`)
- **Type** for unions and computed types (e.g., `HealthStatus`)
- **Enum** for fixed value sets matching proto enums (e.g., `ErrorCode`)

## Synchronization with Proto Schema

**IMPORTANT:** These types must stay in sync with `proto/monoterminal/v1/messages.proto`.

When the proto schema changes:

1. **Update the corresponding TypeScript file** (protocol/health/dashboard)
2. **Run type checks:** `npm run type-check`
3. **Update tests** if message structure changes
4. **Update this README** if new categories are added

### Automated Sync (Future)

Phase 2 may introduce `protobufjs` code generation to auto-generate these types. For Phase 1, manual sync is acceptable given the stable schema.

## Testing

Type definitions are verified through:

1. **TypeScript compiler** - Catches structural errors
2. **Unit tests** - `*.test.ts` files import and use these types
3. **Integration tests** - WebSocket client validates against real messages

Run type checks:
```bash
npm run type-check
```

## Phase Roadmap

### Phase 1 (Current)
- ✅ Core protocol types (Attach, I/O, Resize, Error)
- ✅ Health check & upgrade types
- ✅ Dashboard & monitoring types
- ✅ Detection types

### Phase 2 (Future)
- P2P types (WebRTC DataChannel, peer discovery)
- Compression utilities (zstd integration)
- Auto-generated types from `.proto` files

### Phase 3+
- Platform-specific types (if needed)
- Advanced monitoring/telemetry types

## See Also

- [Protocol Buffers Schema](../../../../proto/monoterminal/v1/messages.proto)
- [SRS §2.4 - Monomind Integration](../../../../docs/monoterminal-srs.md#24-monomind-deep-integration)
- [SRS §3.1.1 - Wire Protocol](../../../../docs/monoterminal-srs.md#311-wire-protocol)
