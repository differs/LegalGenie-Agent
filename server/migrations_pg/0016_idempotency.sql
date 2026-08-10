-- Idempotent write operations (P0): keyed replay of successful responses.
CREATE TABLE IF NOT EXISTS idempotency_records (
    id            BIGSERIAL PRIMARY KEY,
    request_key   TEXT NOT NULL UNIQUE,
    status_code   BIGINT NOT NULL,
    response_body TEXT NOT NULL,
    created_at    TEXT NOT NULL DEFAULT utc_text()
);
CREATE INDEX IF NOT EXISTS idx_idempotency_records_created_at
    ON idempotency_records (created_at);
