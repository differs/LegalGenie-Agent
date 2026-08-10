-- Persons + links to cases (minimal MVP).
CREATE TABLE IF NOT EXISTS persons (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    gender       TEXT,
    phone        TEXT,
    email        TEXT,
    organization TEXT,
    position     TEXT,
    notes        TEXT,
    status       TEXT NOT NULL DEFAULT 'active',
    created_by   TEXT NOT NULL REFERENCES users(id),
    created_at   TEXT NOT NULL DEFAULT utc_text(),
    updated_at   TEXT NOT NULL DEFAULT utc_text()
);

CREATE INDEX IF NOT EXISTS idx_persons_name ON persons(name);
CREATE INDEX IF NOT EXISTS idx_persons_phone ON persons(phone);
CREATE INDEX IF NOT EXISTS idx_persons_status ON persons(status);

CREATE TABLE IF NOT EXISTS person_case_links (
    id            TEXT PRIMARY KEY,
    person_id     TEXT NOT NULL REFERENCES persons(id) ON DELETE CASCADE,
    case_id       TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
    role_type     TEXT NOT NULL,
    role_detail   TEXT,
    involved_date TEXT,
    created_at    TEXT NOT NULL DEFAULT utc_text(),
    UNIQUE(person_id, case_id, role_type)
);

CREATE INDEX IF NOT EXISTS idx_person_case_links_person ON person_case_links(person_id);
CREATE INDEX IF NOT EXISTS idx_person_case_links_case ON person_case_links(case_id);
CREATE INDEX IF NOT EXISTS idx_person_case_links_role ON person_case_links(role_type);
