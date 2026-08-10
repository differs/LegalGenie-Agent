-- Full-text search: trigram (pg_trgm) mirror tables replace FTS5.
-- Each mirror keeps a denormalized `searchable` column and is kept in sync
-- with triggers; ILIKE '%kw%' + GIN trgm index handles CJK and latin text.

CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE TABLE IF NOT EXISTS search_cases (
    rowid      TEXT PRIMARY KEY,
    name       TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    tags       TEXT NOT NULL DEFAULT '',
    searchable TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS search_cases_trgm_idx
    ON search_cases USING GIN (searchable gin_trgm_ops);

CREATE OR REPLACE FUNCTION fn_search_cases_sync()
RETURNS trigger AS $$
BEGIN
  IF TG_OP = 'DELETE' THEN
    DELETE FROM search_cases WHERE rowid = OLD.id;
    RETURN OLD;
  END IF;
  INSERT INTO search_cases (rowid, name, description, tags, searchable)
  VALUES (
    NEW.id, NEW.name, COALESCE(NEW.description, ''), COALESCE(NEW.tags, ''),
    COALESCE(NEW.name, '') || ' ' || COALESCE(NEW.description, '') || ' ' || COALESCE(NEW.tags, '')
  )
  ON CONFLICT (rowid) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    tags = EXCLUDED.tags,
    searchable = EXCLUDED.searchable;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS cases_search_sync ON cases;
CREATE TRIGGER cases_search_sync
AFTER INSERT OR UPDATE OR DELETE ON cases
FOR EACH ROW EXECUTE FUNCTION fn_search_cases_sync();

CREATE TABLE IF NOT EXISTS search_evidence (
    rowid       TEXT PRIMARY KEY,
    original_name TEXT NOT NULL DEFAULT '',
    parsed_text TEXT NOT NULL DEFAULT '',
    searchable  TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS search_evidence_trgm_idx
    ON search_evidence USING GIN (searchable gin_trgm_ops);
CREATE INDEX IF NOT EXISTS search_evidence_trgm_name_idx
    ON search_evidence USING GIN (original_name gin_trgm_ops);
CREATE INDEX IF NOT EXISTS search_evidence_trgm_parsed_idx
    ON search_evidence USING GIN (parsed_text gin_trgm_ops);

CREATE OR REPLACE FUNCTION fn_search_evidence_sync()
RETURNS trigger AS $$
BEGIN
  IF TG_OP = 'DELETE' THEN
    DELETE FROM search_evidence WHERE rowid = OLD.id;
    RETURN OLD;
  END IF;
  INSERT INTO search_evidence (rowid, original_name, parsed_text, searchable)
  VALUES (
    NEW.id, NEW.original_name, COALESCE(NEW.parsed_text, ''),
    COALESCE(NEW.original_name, '') || ' ' || COALESCE(NEW.parsed_text, '')
  )
  ON CONFLICT (rowid) DO UPDATE SET
    original_name = EXCLUDED.original_name,
    parsed_text = EXCLUDED.parsed_text,
    searchable = EXCLUDED.searchable;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS evidence_files_search_sync ON evidence_files;
CREATE TRIGGER evidence_files_search_sync
AFTER INSERT OR UPDATE OR DELETE ON evidence_files
FOR EACH ROW EXECUTE FUNCTION fn_search_evidence_sync();

CREATE TABLE IF NOT EXISTS search_nodes (
    rowid       TEXT PRIMARY KEY,
    title       TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    tags        TEXT NOT NULL DEFAULT '',
    searchable  TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS search_nodes_trgm_idx
    ON search_nodes USING GIN (searchable gin_trgm_ops);

CREATE OR REPLACE FUNCTION fn_search_nodes_sync()
RETURNS trigger AS $$
BEGIN
  IF TG_OP = 'DELETE' THEN
    DELETE FROM search_nodes WHERE rowid = OLD.id;
    RETURN OLD;
  END IF;
  INSERT INTO search_nodes (rowid, title, description, tags, searchable)
  VALUES (
    NEW.id, NEW.title, COALESCE(NEW.description, ''), COALESCE(NEW.tags, ''),
    COALESCE(NEW.title, '') || ' ' || COALESCE(NEW.description, '') || ' ' || COALESCE(NEW.tags, '')
  )
  ON CONFLICT (rowid) DO UPDATE SET
    title = EXCLUDED.title,
    description = EXCLUDED.description,
    tags = EXCLUDED.tags,
    searchable = EXCLUDED.searchable;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS event_nodes_search_sync ON event_nodes;
CREATE TRIGGER event_nodes_search_sync
AFTER INSERT OR UPDATE OR DELETE ON event_nodes
FOR EACH ROW EXECUTE FUNCTION fn_search_nodes_sync();

-- Search history (per user)
CREATE TABLE IF NOT EXISTS search_histories (
    id           TEXT PRIMARY KEY,
    user_id      TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    keyword      TEXT NOT NULL,
    object_types TEXT,
    case_id      TEXT,
    result_count BIGINT,
    searched_at  TEXT NOT NULL DEFAULT utc_text()
);

CREATE INDEX IF NOT EXISTS idx_search_histories_user_time
  ON search_histories(user_id, searched_at DESC);
CREATE INDEX IF NOT EXISTS idx_search_histories_keyword
  ON search_histories(keyword);

-- Hot searches (global)
CREATE TABLE IF NOT EXISTS hot_searches (
    keyword       TEXT PRIMARY KEY,
    search_count  BIGINT NOT NULL DEFAULT 0,
    last_searched TEXT,
    updated_at    TEXT NOT NULL DEFAULT utc_text()
);
