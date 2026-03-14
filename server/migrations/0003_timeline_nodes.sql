CREATE TABLE IF NOT EXISTS event_nodes (
    id          TEXT PRIMARY KEY,
    case_id     TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
    title       TEXT NOT NULL,
    description TEXT,
    event_time  TEXT NOT NULL, -- YYYY-MM-DD
    sort_order  INTEGER NOT NULL DEFAULT 0,
    tags        TEXT, -- JSON array
    status      TEXT NOT NULL DEFAULT 'active',
    created_by  TEXT NOT NULL REFERENCES users(id),
    created_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_event_nodes_case ON event_nodes(case_id);
CREATE INDEX IF NOT EXISTS idx_event_nodes_case_time ON event_nodes(case_id, event_time);
CREATE INDEX IF NOT EXISTS idx_event_nodes_status ON event_nodes(status);

CREATE TABLE IF NOT EXISTS node_evidence_links (
    id          TEXT PRIMARY KEY,
    node_id     TEXT NOT NULL REFERENCES event_nodes(id) ON DELETE CASCADE,
    evidence_id TEXT NOT NULL REFERENCES evidence_files(id) ON DELETE CASCADE,
    anchor_type TEXT NOT NULL,
    anchor_data TEXT,
    created_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_node_evidence_links_node ON node_evidence_links(node_id);
CREATE INDEX IF NOT EXISTS idx_node_evidence_links_evidence ON node_evidence_links(evidence_id);

