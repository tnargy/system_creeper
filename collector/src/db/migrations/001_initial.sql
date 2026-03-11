CREATE TABLE IF NOT EXISTS agents (
    agent_id       TEXT    PRIMARY KEY NOT NULL,
    first_seen_at  TEXT    NOT NULL,
    last_seen_at   TEXT    NOT NULL,
    duplicate_flag INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS metrics (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id           TEXT    NOT NULL REFERENCES agents(agent_id),
    timestamp          TEXT    NOT NULL,
    cpu_percent        REAL    NOT NULL,
    memory_used_bytes  INTEGER NOT NULL,
    memory_total_bytes INTEGER NOT NULL,
    memory_percent     REAL    NOT NULL,
    network_bytes_in   INTEGER NOT NULL,
    network_bytes_out  INTEGER NOT NULL,
    uptime_seconds     INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS disk_readings (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    metric_id   INTEGER NOT NULL REFERENCES metrics(id) ON DELETE CASCADE,
    mount_point TEXT    NOT NULL,
    used_bytes  INTEGER NOT NULL,
    total_bytes INTEGER NOT NULL,
    percent     REAL    NOT NULL
);

CREATE TABLE IF NOT EXISTS thresholds (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id       TEXT REFERENCES agents(agent_id),
    metric_name    TEXT NOT NULL,
    warning_value  REAL NOT NULL,
    critical_value REAL NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_metrics_agent_ts ON metrics (agent_id, timestamp);
