-- Export records (generated artifacts).

CREATE TABLE IF NOT EXISTS export_records (
    id           TEXT PRIMARY KEY,
    case_id       TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
    export_type   TEXT NOT NULL, -- evidence_list/timeline/report
    file_name     TEXT NOT NULL,
    storage_path  TEXT NOT NULL,
    file_size     INTEGER,
    generated_by  TEXT NOT NULL REFERENCES users(id),
    generated_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    node_ids      TEXT, -- JSON array of node ids
    evidence_ids  TEXT  -- JSON array of evidence ids
);

CREATE INDEX IF NOT EXISTS idx_export_records_case_time
  ON export_records(case_id, generated_at DESC);

