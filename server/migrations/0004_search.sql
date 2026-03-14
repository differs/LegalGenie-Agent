-- Full-text search (FTS5) indexes + search history.

-- Cases
CREATE VIRTUAL TABLE IF NOT EXISTS search_cases USING fts5(
    name,
    description,
    tags,
    content='cases',
    content_rowid='rowid',
    tokenize='unicode61'
);

CREATE TRIGGER IF NOT EXISTS cases_ai AFTER INSERT ON cases BEGIN
  INSERT INTO search_cases(rowid, name, description, tags)
  VALUES (NEW.rowid, NEW.name, NEW.description, NEW.tags);
END;

CREATE TRIGGER IF NOT EXISTS cases_ad AFTER DELETE ON cases BEGIN
  INSERT INTO search_cases(search_cases, rowid, name, description, tags)
  VALUES('delete', OLD.rowid, OLD.name, OLD.description, OLD.tags);
END;

CREATE TRIGGER IF NOT EXISTS cases_au AFTER UPDATE ON cases BEGIN
  INSERT INTO search_cases(search_cases, rowid, name, description, tags)
  VALUES('delete', OLD.rowid, OLD.name, OLD.description, OLD.tags);
  INSERT INTO search_cases(rowid, name, description, tags)
  VALUES (NEW.rowid, NEW.name, NEW.description, NEW.tags);
END;

-- Evidence files
CREATE VIRTUAL TABLE IF NOT EXISTS search_evidence USING fts5(
    original_name,
    parsed_text,
    content='evidence_files',
    content_rowid='rowid',
    tokenize='unicode61'
);

CREATE TRIGGER IF NOT EXISTS evidence_files_ai AFTER INSERT ON evidence_files BEGIN
  INSERT INTO search_evidence(rowid, original_name, parsed_text)
  VALUES (NEW.rowid, NEW.original_name, NEW.parsed_text);
END;

CREATE TRIGGER IF NOT EXISTS evidence_files_ad AFTER DELETE ON evidence_files BEGIN
  INSERT INTO search_evidence(search_evidence, rowid, original_name, parsed_text)
  VALUES('delete', OLD.rowid, OLD.original_name, OLD.parsed_text);
END;

CREATE TRIGGER IF NOT EXISTS evidence_files_au AFTER UPDATE ON evidence_files BEGIN
  INSERT INTO search_evidence(search_evidence, rowid, original_name, parsed_text)
  VALUES('delete', OLD.rowid, OLD.original_name, OLD.parsed_text);
  INSERT INTO search_evidence(rowid, original_name, parsed_text)
  VALUES (NEW.rowid, NEW.original_name, NEW.parsed_text);
END;

-- Timeline nodes
CREATE VIRTUAL TABLE IF NOT EXISTS search_nodes USING fts5(
    title,
    description,
    tags,
    content='event_nodes',
    content_rowid='rowid',
    tokenize='unicode61'
);

CREATE TRIGGER IF NOT EXISTS event_nodes_ai AFTER INSERT ON event_nodes BEGIN
  INSERT INTO search_nodes(rowid, title, description, tags)
  VALUES (NEW.rowid, NEW.title, NEW.description, NEW.tags);
END;

CREATE TRIGGER IF NOT EXISTS event_nodes_ad AFTER DELETE ON event_nodes BEGIN
  INSERT INTO search_nodes(search_nodes, rowid, title, description, tags)
  VALUES('delete', OLD.rowid, OLD.title, OLD.description, OLD.tags);
END;

CREATE TRIGGER IF NOT EXISTS event_nodes_au AFTER UPDATE ON event_nodes BEGIN
  INSERT INTO search_nodes(search_nodes, rowid, title, description, tags)
  VALUES('delete', OLD.rowid, OLD.title, OLD.description, OLD.tags);
  INSERT INTO search_nodes(rowid, title, description, tags)
  VALUES (NEW.rowid, NEW.title, NEW.description, NEW.tags);
END;

-- Search history (per user)
CREATE TABLE IF NOT EXISTS search_histories (
    id           TEXT PRIMARY KEY,
    user_id      TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    keyword      TEXT NOT NULL,
    object_types TEXT,
    case_id      TEXT,
    result_count INTEGER,
    searched_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_search_histories_user_time
  ON search_histories(user_id, searched_at DESC);

CREATE INDEX IF NOT EXISTS idx_search_histories_keyword
  ON search_histories(keyword);

-- Hot searches (global)
CREATE TABLE IF NOT EXISTS hot_searches (
    keyword       TEXT PRIMARY KEY,
    search_count  INTEGER NOT NULL DEFAULT 0,
    last_searched DATETIME,
    updated_at    DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Build indexes for any pre-existing data (safe no-op on empty DB).
INSERT INTO search_cases(search_cases) VALUES('rebuild');
INSERT INTO search_evidence(search_evidence) VALUES('rebuild');
INSERT INTO search_nodes(search_nodes) VALUES('rebuild');
