CREATE TABLE IF NOT EXISTS export_records (
    id           TEXT PRIMARY KEY,
    case_id       TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
    export_type   TEXT NOT NULL,
    file_name     TEXT NOT NULL,
    storage_path  TEXT NOT NULL,
    file_size     BIGINT,
    generated_by  TEXT NOT NULL REFERENCES users(id),
    generated_at  TEXT NOT NULL DEFAULT utc_text(),
    node_ids      TEXT,
    evidence_ids  TEXT
);

CREATE INDEX IF NOT EXISTS idx_export_records_case_time
  ON export_records(case_id, generated_at DESC);
