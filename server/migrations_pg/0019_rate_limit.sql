-- Shared rate limiting (P3): multi-instance safe counters in PostgreSQL.
CREATE TABLE IF NOT EXISTS rate_limit_counters (
    key          TEXT PRIMARY KEY,
    count        BIGINT NOT NULL DEFAULT 0,
    window_start TEXT NOT NULL
);

-- Permission matrix (P3): role x operation policy table.
-- Seed matches the historical owner/member/viewer semantics; operators can
-- tighten rows without code changes (e.g. revoke member 'case:export').
CREATE TABLE IF NOT EXISTS permissions (
    role      TEXT NOT NULL,
    operation TEXT NOT NULL,
    PRIMARY KEY (role, operation)
);

INSERT INTO permissions (role, operation) VALUES
    ('owner',  'case:read'),
    ('owner',  'case:write'),
    ('owner',  'case:manage'),
    ('owner',  'case:export'),
    ('owner',  'case:member'),
    ('member', 'case:read'),
    ('member', 'case:write'),
    ('member', 'case:export'),
    ('viewer', 'case:read')
ON CONFLICT (role, operation) DO NOTHING;
