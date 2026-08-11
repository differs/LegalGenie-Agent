-- Agent runtime (P2): server-side sessions, messages, and tool calls.
--
-- The agent executes tools through a unified pipeline; write/sensitive
-- tools create approval requests that a human confirms before execution.
CREATE TABLE IF NOT EXISTS agent_sessions (
    id         TEXT PRIMARY KEY,
    case_id    TEXT REFERENCES cases(id) ON DELETE CASCADE,
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title      TEXT NOT NULL DEFAULT 'New session',
    status     TEXT NOT NULL DEFAULT 'active', -- active|archived
    created_at TEXT NOT NULL DEFAULT utc_text(),
    updated_at TEXT NOT NULL DEFAULT utc_text()
);

CREATE INDEX IF NOT EXISTS idx_agent_sessions_user
    ON agent_sessions (user_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS agent_messages (
    id         TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE,
    role       TEXT NOT NULL, -- user|assistant
    content    TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT utc_text()
);

CREATE INDEX IF NOT EXISTS idx_agent_messages_session
    ON agent_messages (session_id, created_at ASC);

CREATE TABLE IF NOT EXISTS agent_tool_calls (
    id                 TEXT PRIMARY KEY,
    session_id         TEXT NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE,
    tool_name          TEXT NOT NULL,
    input              TEXT NOT NULL,   -- JSON
    output             TEXT,            -- JSON
    status             TEXT NOT NULL DEFAULT 'pending', -- pending|approved|rejected|executed|failed
    requires_approval  BOOLEAN NOT NULL DEFAULT FALSE,
    danger_level       TEXT NOT NULL DEFAULT 'read',    -- read|write|sensitive|destructive
    error              TEXT,
    requested_by       TEXT NOT NULL REFERENCES users(id),
    decided_by         TEXT,
    decided_at         TEXT,
    created_at         TEXT NOT NULL DEFAULT utc_text()
);

CREATE INDEX IF NOT EXISTS idx_agent_tool_calls_session
    ON agent_tool_calls (session_id, created_at ASC);
CREATE INDEX IF NOT EXISTS idx_agent_tool_calls_pending
    ON agent_tool_calls (status, created_at ASC);
