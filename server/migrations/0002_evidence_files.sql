CREATE TABLE IF NOT EXISTS evidence_files (
    id            TEXT PRIMARY KEY,
    case_id        TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
    original_name  TEXT NOT NULL,
    stored_name    TEXT NOT NULL,
    file_type      TEXT NOT NULL,
    file_size      INTEGER NOT NULL,
    storage_path   TEXT NOT NULL,
    parsed_text    TEXT,
    page_count     INTEGER,
    duration       INTEGER,
    metadata       TEXT,
    status         TEXT NOT NULL DEFAULT 'active',
    uploaded_by    TEXT NOT NULL REFERENCES users(id),
    created_at     DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_evidence_files_case ON evidence_files(case_id);
CREATE INDEX IF NOT EXISTS idx_evidence_files_status ON evidence_files(status);
CREATE INDEX IF NOT EXISTS idx_evidence_files_created_at ON evidence_files(created_at);

