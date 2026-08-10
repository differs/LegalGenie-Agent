-- Idempotent write operations (P0): keyed replay of successful responses.
CREATE TABLE IF NOT EXISTS idempotency_records (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    request_key   TEXT NOT NULL UNIQUE,
    status_code   INTEGER NOT NULL,
    response_body TEXT NOT NULL,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_idempotency_records_created_at
    ON idempotency_records (created_at);
