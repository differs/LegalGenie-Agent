-- Person-person relationships within a case (MVP).

CREATE TABLE IF NOT EXISTS person_relationships (
    id             TEXT PRIMARY KEY,
    case_id         TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
    from_person_id  TEXT NOT NULL REFERENCES persons(id) ON DELETE CASCADE,
    to_person_id    TEXT NOT NULL REFERENCES persons(id) ON DELETE CASCADE,
    rel_type        TEXT NOT NULL,
    rel_detail      TEXT,
    status          TEXT NOT NULL DEFAULT 'active',
    created_by      TEXT NOT NULL REFERENCES users(id),
    created_at      DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at      DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (from_person_id != to_person_id),
    UNIQUE(case_id, from_person_id, to_person_id, rel_type)
);

CREATE INDEX IF NOT EXISTS idx_person_relationships_case ON person_relationships(case_id);
CREATE INDEX IF NOT EXISTS idx_person_relationships_from ON person_relationships(from_person_id);
CREATE INDEX IF NOT EXISTS idx_person_relationships_to ON person_relationships(to_person_id);
CREATE INDEX IF NOT EXISTS idx_person_relationships_status ON person_relationships(status);

