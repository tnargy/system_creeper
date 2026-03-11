# System Creeper — Feature Punchlist

> **How to use this document:** Each ticket is a self-contained unit of work. Hand individual tickets (or a group) to an implementing agent along with SPEC.md, DESIGN.md, and the relevant section of the architecture notes in plan.md. Tickets are sequenced by dependency — do not skip ahead.

---

## Phase 1 — Workspace Scaffold

### TICKET-001 — Cargo Workspace Root
**Status:** Complete
**Depends on:** nothing
**Files to create:**
- `Cargo.toml` (workspace root)
- `Cargo.lock`
- `.gitignore`

**Acceptance criteria:**
- `Cargo.toml` declares a `[workspace]` with members: `["shared", "agent", "collector"]`
- Running `cargo build --workspace` succeeds with an empty workspace (no crates yet, just structure)
- `.gitignore` excludes `/target`, `*.db`, `.env`, and `dashboard/node_modules`

---

### TICKET-002 — `shared` Library Crate
**Status:** Complete
**Depends on:** TICKET-001
**Files to create:**
- `shared/Cargo.toml`
- `shared/src/lib.rs`

**Acceptance criteria:**
- Crate compiles with `cargo build -p shared`
- Exports exactly four public structs: `MetricPayload`, `MemoryInfo`, `DiskInfo`, `NetworkInfo`
- All structs derive `Debug`, `Serialize`, `Deserialize`
- `MetricPayload.timestamp` is `chrono::DateTime<Utc>`
- All integer byte fields are `u64`; all percent fields are `f64`
- No logic, no `main`, no HTTP code — types only

**Dependencies (Cargo):**
```toml
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
```

---

## Phase 2 — Collector

### TICKET-003 — Collector Crate Skeleton + Config
**Status:** Complete
**Depends on:** TICKET-002
**Files to create:**
- `collector/Cargo.toml`
- `collector/collector.example.toml`
- `collector/src/main.rs`
- `collector/src/config.rs`

**Acceptance criteria:**
- Crate compiles and binary starts, prints startup log line with `listen_addr`, then exits cleanly
- Config struct fields: `listen_addr` (String), `database_path` (String), `offline_threshold_secs` (u64, default 120), `retention_days` (u32, default 30), `log_level` (String, default `"info"`)
- If the TOML file path is given as a CLI argument and the file is missing or malformed, the process exits with a non-zero code and a human-readable error
- `collector.example.toml` matches the schema in plan.md exactly

**Dependencies (Cargo):**
```toml
axum = { version = "0.8", features = ["ws"] }
tokio = { version = "1", features = ["full"] }
sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio", "migrate", "chrono"] }
tower-http = { version = "0.6", features = ["cors"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
shared = { path = "../shared" }
```

---

### TICKET-004 — Collector Database Layer + Migrations
**Status:** Complete
**Depends on:** TICKET-003
**Files to create:**
- `collector/src/db/mod.rs`
- `collector/src/db/queries.rs`
- `collector/src/db/migrations/001_initial.sql`

**Acceptance criteria:**
- On startup, the collector runs pending migrations automatically via sqlx's built-in migrate macro
- Migration 001 creates all four tables exactly as specified in plan.md: `agents`, `metrics`, `disk_readings`, `thresholds`
- Index `idx_metrics_agent_ts` on `(agent_id, timestamp)` is created
- `disk_readings` has `ON DELETE CASCADE` referencing `metrics(id)`
- `db/queries.rs` exposes typed async functions (not raw SQL strings in handlers):
  - `upsert_agent(pool, agent_id, timestamp)` — insert or update `agents` row
  - `insert_metric(pool, payload) -> metric_id` — insert into `metrics`, return the new `id`
  - `insert_disk_readings(pool, metric_id, disks)` — bulk insert into `disk_readings`
  - `get_agents_summary(pool) -> Vec<AgentSummary>` — join agents + latest metric snapshot
  - `get_snapshot(pool, agent_id) -> Option<MetricSnapshot>`
  - `get_history(pool, agent_id, since: DateTime<Utc>) -> Vec<MetricSnapshot>`
  - `get_thresholds(pool) -> Vec<Threshold>`
  - `upsert_threshold(pool, ...)` / `update_threshold(pool, id, ...)` / `delete_threshold(pool, id)`
  - `delete_old_metrics(pool, cutoff: DateTime<Utc>)` — used by retention task
- Running the collector against an empty database creates the schema without error

---

### TICKET-005 — Collector: Metric Ingest Endpoint
**Status:** Complete
**Depends on:** TICKET-004
**Files to create / modify:**
- `collector/src/api/mod.rs`
- `collector/src/api/ingest.rs`
- `collector/src/main.rs` (wire the router)

**Acceptance criteria:**
- `POST /api/v1/metrics` with a valid `MetricPayload` JSON body:
  - Upserts the agent record (sets `last_seen_at`; sets `first_seen_at` only on first appearance)
  - Inserts metric row + disk readings in a single transaction
  - Returns `200 OK` with an empty body
- `POST /api/v1/metrics` with a malformed body (missing required field, wrong type): returns `400 Bad Request`
- When the database is unavailable: returns `503 Service Unavailable`
- Concurrent POSTs from multiple agents do not produce data corruption or panics (test with 10 simultaneous requests)
- CORS headers are present on all responses (allows `*` origin for development)

---

### TICKET-006 — Collector: Read Endpoints (Agents, Snapshots, History, Thresholds)
**Status:** Complete
**Depends on:** TICKET-005
**Files to create:**
- `collector/src/api/agents.rs`
- `collector/src/api/thresholds.rs`

**Acceptance criteria:**

`GET /api/v1/agents`
- Returns JSON array of `AgentSummary` objects
- Each item includes: `agent_id`, `status` (computed: `online|warning|critical|offline`), `last_seen_at`, `duplicate_flag`, and the latest metric snapshot (cpu, memory, disk, network, uptime)
- `status` is `offline` if `now - last_seen_at > offline_threshold_secs`; otherwise the worst metric state vs. thresholds
- Empty array returned (not 404) when no agents exist yet

`GET /api/v1/agents/:agent_id/snapshot`
- Returns the single most recent `MetricSnapshot` for the agent
- 404 if agent unknown

`GET /api/v1/agents/:agent_id/history?range=1h|6h|24h|7d`
- Returns array of `MetricSnapshot` ordered by `timestamp ASC`
- Defaults to `1h` if `range` param is absent or unrecognized
- Results are subsampled to a maximum of 300 data points for large ranges (drop every Nth row to fit the cap)
- Returns empty array (not 404) if agent exists but no history in the requested window

`GET /api/v1/thresholds`
- Returns all rows from the `thresholds` table as JSON array

`POST /api/v1/thresholds`
- Body: `{ agent_id?: string|null, metric_name: string, warning_value: number, critical_value: number }`
- `metric_name` must be one of `"cpu"`, `"memory"`, `"disk"`; returns 400 otherwise
- Returns 201 with the created row

`PUT /api/v1/thresholds/:id`
- Body: `{ warning_value: number, critical_value: number }`
- Returns 200 with the updated row; 404 if id not found

`DELETE /api/v1/thresholds/:id`
- Returns 204; 404 if not found

---

### TICKET-007 — Collector: WebSocket Push
**Status:** Complete
**Depends on:** TICKET-005
**Files to create:**
- `collector/src/api/ws.rs`

**Acceptance criteria:**
- `GET /ws` upgrades to a WebSocket connection
- After each successful metric persist (TICKET-005), the collector immediately broadcasts a `metric_update` JSON message to every connected WebSocket client
- Message schema matches exactly the structure in plan.md (all fields present, `status` computed at emit time)
- Clients that disconnect mid-session are dropped from the broadcast set without panicking other connections or leaking memory
- A `tokio::sync::broadcast::channel` (or equivalent) is used for fan-out; the ingest handler sends, the WS handler receives and forwards
- If no clients are connected, the broadcast completes without blocking or erroring

---

### TICKET-008 — Collector: Retention Task
**Status:** Complete
**Depends on:** TICKET-004
**Files to create / modify:**
- `collector/src/retention.rs`
- `collector/src/main.rs` (spawn the task)

**Acceptance criteria:**
- A background `tokio` task runs once per 24 hours
- On each run, it deletes all rows from `metrics` where `timestamp < now - retention_days`
- Cascading `ON DELETE CASCADE` on `disk_readings` removes associated disk rows automatically — no separate disk delete query needed
- If the deletion fails (DB error), the task logs the error and schedules the next attempt normally (does not crash the process)
- First run occurs at startup (runs once immediately, then waits 24h between subsequent runs)

---

## Phase 3 — Agent

### TICKET-009 — Agent Crate Skeleton + Config
**Status:** Not Started
**Depends on:** TICKET-002
**Files to create:**
- `agent/Cargo.toml`
- `agent/agent.example.toml`
- `agent/src/main.rs`
- `agent/src/config.rs`

**Acceptance criteria:**
- Crate compiles and binary starts, prints startup log line, then exits cleanly
- Config struct fields: `agent_id` (Option<String>), `collector_url` (String), `interval_secs` (u64, default 30), `buffer_duration_secs` (u64, default 300), `log_level` (String, default `"info"`)
- If `agent_id` is absent or empty string, the agent sets it to the machine's hostname at runtime
- If the config file is missing or malformed, the agent exits with a non-zero code and a human-readable error — no silent default startup
- `agent.example.toml` matches the schema in plan.md exactly

**Dependencies (Cargo):**
```toml
sysinfo = "0.33"
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
shared = { path = "../shared" }
```

---

### TICKET-010 — Agent: Metric Collection
**Status:** Not Started
**Depends on:** TICKET-009
**Files to create:**
- `agent/src/metrics.rs`

**Acceptance criteria:**
- `collect_metrics(agent_id: &str) -> MetricPayload` function (sync or async)
- Uses `sysinfo` crate to collect:
  - `cpu_percent`: overall CPU usage averaged across all cores (0.0–100.0)
  - `memory`: used/total bytes and percent
  - `disks`: all mounted, readable volumes with used/total bytes and percent per mount point
  - `network`: bytes received and bytes sent since last sample (delta, not cumulative totals — requires storing previous sample state)
  - `uptime_seconds`: system uptime in integer seconds
- `timestamp` is `chrono::Utc::now()` at collection time
- Disk entries with `total_bytes == 0` are excluded (avoids divide-by-zero in percent)
- Network delta: first sample reports `0` for both directions (no previous reference); subsequent samples report the delta

---

### TICKET-011 — Agent: HTTP Sender with Retry Buffer
**Status:** Not Started
**Depends on:** TICKET-009, TICKET-010
**Files to create:**
- `agent/src/sender.rs`

**Acceptance criteria:**
- `send_with_retry(client, url, buffer, new_payload)` function (or equivalent struct/loop)
- On each interval: appends `new_payload` to the in-memory `VecDeque` retry buffer, then attempts to send all buffered payloads oldest-first
- On a successful `200` response for a payload: removes it from the buffer
- On a non-2xx response or network error: stops sending for this interval (does not retry the rest of the queue until the next interval), logs the failure
- When the buffer exceeds `buffer_duration_secs` worth of payloads (calculated as `buffer_duration_secs / interval_secs` entries): drops the oldest entry and logs a warning — the agent never crashes due to buffer overflow
- The main agent loop: starts collection ticker, calls `collect_metrics`, then calls `send_with_retry` each tick
- Agent runs indefinitely until process is killed; all panics in the collection or send path are caught and logged, not propagated to crash the process

---

## Phase 4 — Dashboard

### TICKET-012 — Dashboard Project Scaffold
**Status:** Not Started
**Depends on:** nothing (can be done in parallel with Phase 2/3)
**Files to create:**
- `dashboard/package.json`
- `dashboard/vite.config.ts`
- `dashboard/tsconfig.json`
- `dashboard/tailwind.config.ts`
- `dashboard/postcss.config.js`
- `dashboard/index.html`
- `dashboard/src/main.tsx`
- `dashboard/src/App.tsx` (stub)
- `dashboard/src/types/index.ts`

**Acceptance criteria:**
- `npm install` completes without errors
- `npm run dev` starts Vite dev server and serves a blank page with the app title "System Creeper"
- TypeScript config is strict (`"strict": true`)
- Tailwind CSS is active (a test `className="text-red-500"` renders in red)

**Dependencies (`package.json`):**
```json
"react": "^18",
"react-dom": "^18",
"typescript": "^5",
"vite": "^5",
"@vitejs/plugin-react": "^4",
"tailwindcss": "^3",
"autoprefixer": "^10",
"postcss": "^8",
"recharts": "^2",
"@tanstack/react-query": "^5",
"lucide-react": "latest"
```

**Types to define in `src/types/index.ts`:**
```ts
type AgentStatus = 'online' | 'warning' | 'critical' | 'offline'

interface DiskInfo { mount_point: string; used_bytes: number; total_bytes: number; percent: number }
interface NetworkInfo { bytes_in: number; bytes_out: number }
interface MemoryInfo { used_bytes: number; total_bytes: number; percent: number }

interface MetricSnapshot {
  timestamp: string
  cpu_percent: number
  memory: MemoryInfo
  disks: DiskInfo[]
  network: NetworkInfo
  uptime_seconds: number
}

interface AgentSummary {
  agent_id: string
  status: AgentStatus
  last_seen_at: string
  duplicate_flag: boolean
  snapshot: MetricSnapshot | null
}

interface Threshold {
  id: number
  agent_id: string | null
  metric_name: 'cpu' | 'memory' | 'disk'
  warning_value: number
  critical_value: number
}

interface MetricUpdateEvent {
  event: 'metric_update'
  agent_id: string
  timestamp: string
  status: AgentStatus
  cpu_percent: number
  memory: MemoryInfo
  disks: DiskInfo[]
  network: NetworkInfo
  uptime_seconds: number
  duplicate_flag: boolean
}
```

---

### TICKET-013 — Dashboard: REST + WebSocket Client
**Status:** Not Started
**Depends on:** TICKET-012
**Files to create:**
- `dashboard/src/api/client.ts`

**Acceptance criteria:**
- Exports typed async functions for every REST endpoint:
  - `fetchAgents(): Promise<AgentSummary[]>`
  - `fetchSnapshot(agentId: string): Promise<MetricSnapshot>`
  - `fetchHistory(agentId: string, range: '1h'|'6h'|'24h'|'7d'): Promise<MetricSnapshot[]>`
  - `fetchThresholds(): Promise<Threshold[]>`
  - `createThreshold(body): Promise<Threshold>`
  - `updateThreshold(id: number, body): Promise<Threshold>`
  - `deleteThreshold(id: number): Promise<void>`
- All functions throw a typed error on non-2xx responses
- Base URL defaults to `http://localhost:8080/api/v1`; configurable via `VITE_COLLECTOR_URL` env var
- Exports `createWebSocket(onMessage, onOpen, onClose): WebSocket`
  - WS URL derived from collector base URL (http→ws, https→wss)
  - Calls `onMessage` with parsed `MetricUpdateEvent` on each message
  - Calls `onOpen` / `onClose` as lifecycle callbacks

---

### TICKET-014 — Dashboard: `useWebSocket` Hook
**Status:** Not Started
**Depends on:** TICKET-013
**Files to create:**
- `dashboard/src/hooks/useWebSocket.ts`

**Acceptance criteria:**
- Hook signature: `useWebSocket(onEvent: (e: MetricUpdateEvent) => void): { connected: boolean }`
- Manages WebSocket lifecycle: opens on mount, closes on unmount
- Reconnects automatically on close/error using exponential backoff (start: 1s, max: 30s, multiplier: 2)
- Exposes `connected: boolean` (true only while the socket is in `OPEN` state)
- Does not reconnect if the component has unmounted

---

### TICKET-015 — Dashboard: `useAgents` Hook
**Status:** Not Started
**Depends on:** TICKET-013, TICKET-014
**Files to create:**
- `dashboard/src/hooks/useAgents.ts`

**Acceptance criteria:**
- Uses `@tanstack/react-query` to fetch `fetchAgents()` once on mount
- Subscribes to WebSocket events via a shared `useWebSocket` instance; on each `metric_update` event, updates the matching agent in the query cache in place (no full re-fetch)
- Exposes: `{ agents: AgentSummary[], connected: boolean, isLoading: boolean, error: Error | null }`
- When `connected` transitions from false→true, triggers a re-fetch of agents to get fresh snapshots

---

### TICKET-016 — Dashboard: `StatusBadge` Component
**Status:** Not Started
**Depends on:** TICKET-012
**Files to create:**
- `dashboard/src/components/StatusBadge.tsx`

**Acceptance criteria:**
- Props: `status: AgentStatus`
- Renders a pill-style badge using Lucide icons and status colors from DESIGN.md:
  - `online` → filled green circle + "Online"
  - `warning` → amber triangle + "Warning"
  - `critical` → red X circle + "Critical"
  - `offline` → gray dashed circle + "Offline"
- No external state; purely presentational

---

### TICKET-017 — Dashboard: `AgentCard` Component
**Status:** Not Started
**Depends on:** TICKET-016
**Files to create:**
- `dashboard/src/components/AgentCard.tsx`

**Acceptance criteria:**
- Props: `agent: AgentSummary`, `onClick: () => void`, `disabled: boolean`
- Renders the card anatomy from DESIGN.md:
  - 4px left border colored by status (green/amber/red/gray)
  - Top row: status icon + agent name (bold)
  - Metric rows: CPU %, Memory %, primary disk %, uptime, last-seen timestamp
  - If `agent.snapshot === null` (disconnected or offline): all metric values display as `—`
  - If `agent.status === 'offline'`: shows `OFFLINE` label, last-seen timestamp, no metric values
  - If `agent.duplicate_flag === true`: agent name is `text-red-500`, warning icon prefix, card border is amber regardless of metric status, `title` attribute reads `"Duplicate agent ID detected"`
- Card is not clickable (cursor-default, no onClick fired) when `disabled === true`
- Entire card area is clickable when enabled

---

### TICKET-018 — Dashboard: `DisconnectedBanner` Component
**Status:** Not Started
**Depends on:** TICKET-012
**Files to create:**
- `dashboard/src/components/DisconnectedBanner.tsx`

**Acceptance criteria:**
- Props: `visible: boolean`
- When `visible === false`: renders nothing (not even an empty div)
- When `visible === true`: renders a full-width amber/red banner below the header with the text: `"⚠ Data unavailable. Connection to collector lost. Metric values are hidden until connection is restored."`
- Uses `transition-all` to animate in/out smoothly

---

### TICKET-019 — Dashboard: `Header` Component
**Status:** Not Started
**Depends on:** TICKET-016
**Files to create:**
- `dashboard/src/components/Header.tsx`

**Acceptance criteria:**
- Props: `connected: boolean`
- Left: "System Creeper" in `text-xl font-semibold`
- Right: when `connected === true` → green dot + "Connected"; when false → red X + "DISCONNECTED — reconnecting..." with red background pill
- Uses Lucide `Wifi` / `WifiOff` icons

---

### TICKET-020 — Dashboard: `AgentGrid` View
**Status:** Not Started
**Depends on:** TICKET-017, TICKET-018, TICKET-019, TICKET-015
**Files to create:**
- `dashboard/src/components/AgentGrid.tsx`

**Acceptance criteria:**
- Props: `agents: AgentSummary[]`, `connected: boolean`, `onSelectAgent: (id: string) => void`, `isLoading: boolean`
- Filter bar: dropdown (All / Online / Warning / Critical / Offline) + text search input; both filter client-side instantly
- Responsive CSS grid: `grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5`, `gap-4`
- While `isLoading`: shows a skeleton placeholder grid (5 gray rounded cards)
- While `!connected`: cards are rendered with `disabled={true}` (not clickable) and `snapshot: null`
- Agent count displayed above grid: `Agents (N)` where N reflects the filtered count

---

### TICKET-021 — Dashboard: `MetricChart` Component
**Status:** Not Started
**Depends on:** TICKET-012
**Files to create:**
- `dashboard/src/components/MetricChart.tsx`

**Acceptance criteria:**
- Props:
  ```ts
  interface MetricChartProps {
    data: { timestamp: string; value: number }[]
    yDomain?: [number, number]
    warningValue?: number
    criticalValue?: number
    formatY?: (v: number) => string
    series?: { key: string; color: string }[]  // for dual-line (network)
  }
  ```
- Renders a Recharts `LineChart` (or `ComposedChart`) with a `CartesianGrid`, `XAxis` (time formatted as HH:mm), `YAxis`, and `Tooltip`
- If `warningValue` is set: draws a dashed horizontal `ReferenceLine` in amber (`#f59e0b`)
- If `criticalValue` is set: draws a dashed horizontal `ReferenceLine` in red (`#ef4444`)
- If `series` has two entries (network case): renders two `Line` components on the same chart
- Chart is responsive (`<ResponsiveContainer width="100%" height={180}`)

---

### TICKET-022 — Dashboard: `ThresholdInput` Component
**Status:** Not Started
**Depends on:** TICKET-013
**Files to create:**
- `dashboard/src/components/ThresholdInput.tsx`

**Acceptance criteria:**
- Props: `label: string`, `value: number`, `onSave: (newValue: number) => Promise<void>`
- Renders a labeled `<input type="number">` in monospace font
- On blur or Enter keypress: calls `onSave(newValue)`
  - During save: input is slightly dimmed (`opacity-60`)
  - On success: input briefly flashes a green ring (`ring-2 ring-green-400`) for 600ms
  - On failure: input flashes a red ring, value reverts to the last confirmed value, inline error text appears below the input
- Does not call `onSave` if the value hasn't changed from the confirmed value

---

### TICKET-023 — Dashboard: `MetricPanel` Component
**Status:** Not Started
**Depends on:** TICKET-021, TICKET-022
**Files to create:**
- `dashboard/src/components/MetricPanel.tsx`

**Acceptance criteria:**
- Props:
  ```ts
  interface MetricPanelProps {
    title: string
    currentLabel: string
    currentStatus: AgentStatus | null
    threshold?: { warning: number; critical: number; id?: number }
    chartData: { timestamp: string; value: number }[]
    onThresholdSave?: (field: 'warning' | 'critical', value: number) => Promise<void>
    children?: React.ReactNode  // for disk bar list or network dual-line
  }
  ```
- Renders: panel title, current value + status badge, threshold inputs (if `threshold` is set), `MetricChart`
- Disk panel: `children` receives a bar-per-mount-point list (percent filled bars styled with Tailwind)
- Network panel: current `In: X KB/s` and `Out: Y KB/s` values above a dual-line chart

---

### TICKET-024 — Dashboard: `AgentDetail` View
**Status:** Not Started
**Depends on:** TICKET-023, TICKET-013
**Files to create:**
- `dashboard/src/components/AgentDetail.tsx`

**Acceptance criteria:**
- Props: `agentId: string`, `onBack: () => void`
- Fetches snapshot + thresholds on mount via REST; refetches history when time range selector changes
- Time range selector: pill-style toggle buttons `[1h] [6h] [24h] [7d]`; active is filled, others outlined
- Renders four `MetricPanel` instances: CPU, Memory, Disk, Network
- Header row: `← All Agents` back link + agent name + `StatusBadge` + `Last seen: Xs ago` (live-updating)
- All four charts share the same time range (controlled by the selector)
- Threshold saves call `updateThreshold` or `createThreshold` as appropriate (if no threshold exists for this agent+metric, POST; otherwise PUT)
- Threshold lines on charts reposition immediately on input change (optimistic — don't wait for API response)
- History data is subsampled client-side to ≤300 points if the API returns more (consistency check)

---

### TICKET-025 — Dashboard: `App.tsx` — Top-Level State + View Routing
**Status:** Not Started
**Depends on:** TICKET-020, TICKET-024
**Files to modify:**
- `dashboard/src/App.tsx`

**Acceptance criteria:**
- Manages a single piece of state: `selectedAgentId: string | null`
- When `selectedAgentId === null`: renders `AgentGrid`
- When `selectedAgentId` is set: renders `AgentDetail` with an `onBack` handler that clears it
- `Header` is always rendered above both views
- `DisconnectedBanner` renders between header and content when `connected === false`
- `QueryClientProvider` wraps the entire tree
- The `useAgents` hook lives here (or in `AgentGrid`); `connected` state is passed down to both views and `Header`

---

## Phase 5 — Integration & Hardening

### TICKET-026 — End-to-End Smoke Test (Manual)
**Status:** Not Started
**Depends on:** all previous tickets
**This is a manual verification checklist, not a code ticket.**

1. Start collector: `cargo run -p collector -- collector.toml`
2. Start one agent: `cargo run -p agent -- agent.toml` (point at local collector)
3. Start dashboard: `npm run dev` in `dashboard/`
4. Verify agent card appears within 60 seconds with live metrics
5. Set a CPU warning threshold below current CPU usage → card updates to warning state
6. Kill the agent → card transitions to offline after 2 minutes
7. Restart the agent → card transitions back to online
8. Kill the collector → dashboard shows disconnected banner; metric values hidden
9. Restart the collector → dashboard reconnects; banner disappears
10. Stop the dev server, run `npm run build` → build succeeds with no TypeScript errors

---

### TICKET-027 — Collector: Static File Serving for Dashboard
**Status:** Not Started
**Depends on:** TICKET-025
**Files to modify:**
- `collector/src/main.rs`

**Acceptance criteria:**
- When built with a `--features serve-dashboard` flag (or always), the collector's axum router serves the `dashboard/dist/` folder as static files at `/`
- `GET /` returns `index.html`
- The SPA's client-side navigation works correctly (any unmatched path falls back to `index.html`)
- REST and WebSocket routes are not shadowed by the static file handler (API routes take priority)
- If `dashboard/dist/` does not exist, the collector still starts normally (static serving is gracefully absent)

---

## Reference: Implementation Order

```
TICKET-001 → TICKET-002
                │
    ┌───────────┴──────────────┐
    ▼                          ▼
TICKET-003                TICKET-009
    │                          │
TICKET-004               TICKET-010
    │                          │
TICKET-005               TICKET-011
    │
TICKET-006
    │
TICKET-007
    │
TICKET-008

TICKET-012 (parallel with Rust work)
    │
TICKET-013
    ├── TICKET-014
    │       └── TICKET-015
    ├── TICKET-016
    │       └── TICKET-017
    ├── TICKET-018
    ├── TICKET-019
    └── TICKET-020 (needs 015, 016, 017, 018, 019)

TICKET-021 → TICKET-022 → TICKET-023 → TICKET-024
                                              │
                                         TICKET-025
                                              │
                                         TICKET-026
                                              │
                                         TICKET-027
```

---

## Key Decisions (Do Not Re-Litigate)

| Decision | Choice |
|---|---|
| Dashboard architecture | React SPA — no separate backend server |
| Database | SQLite via sqlx, embedded in collector process |
| Authentication | None — assumed trusted internal network |
| Agent ID source | TOML `agent_id` field; falls back to hostname if absent |
| Real-time transport | WebSocket push from collector; no polling |
| History time ranges | 1h / 6h / 24h / 7d |
| Duplicate ID behavior | Warning icon + red name on card only; no banner or separate view |
| Reporting interval default | 30 seconds |
| Offline threshold default | 120 seconds |
| Retention period default | 30 days |
| Agent buffer duration default | 300 seconds (5 minutes) |
| WS reconnect strategy | Exponential backoff, start 1s, max 30s |
