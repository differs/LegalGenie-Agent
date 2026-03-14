-- Operation logs (audit trail).

CREATE TABLE IF NOT EXISTS operation_logs (
    id             TEXT PRIMARY KEY,
    user_id        TEXT NOT NULL REFERENCES users(id),
    user_name      TEXT NOT NULL,
    case_id        TEXT,
    action         TEXT NOT NULL, -- CREATE/UPDATE/DELETE/LOGIN/...
    module         TEXT NOT NULL, -- case/file/node/user/system
    target_type    TEXT NOT NULL,
    target_id      TEXT,
    target_title   TEXT,
    old_value      TEXT, -- JSON string
    new_value      TEXT, -- JSON string
    changed_fields TEXT,
    ip_address     TEXT,
    user_agent     TEXT,
    request_id     TEXT,
    created_at     DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_operation_logs_user_time
  ON operation_logs(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_operation_logs_case_time
  ON operation_logs(case_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_operation_logs_module_time
  ON operation_logs(module, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_operation_logs_action_time
  ON operation_logs(action, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_operation_logs_target
  ON operation_logs(target_type, target_id);
CREATE INDEX IF NOT EXISTS idx_operation_logs_created_at
  ON operation_logs(created_at DESC);

-- Optional FTS5 index for keyword searches (kept small: no JSON blobs).
CREATE VIRTUAL TABLE IF NOT EXISTS logs_search USING fts5(
    user_name,
    target_title,
    action,
    module,
    changed_fields,
    content='operation_logs',
    content_rowid='rowid',
    tokenize='unicode61'
);

CREATE TRIGGER IF NOT EXISTS operation_logs_ai AFTER INSERT ON operation_logs BEGIN
  INSERT INTO logs_search(rowid, user_name, target_title, action, module, changed_fields)
  VALUES (NEW.rowid, NEW.user_name, NEW.target_title, NEW.action, NEW.module, NEW.changed_fields);
END;

CREATE TRIGGER IF NOT EXISTS operation_logs_ad AFTER DELETE ON operation_logs BEGIN
  INSERT INTO logs_search(logs_search, rowid, user_name, target_title, action, module, changed_fields)
  VALUES('delete', OLD.rowid, OLD.user_name, OLD.target_title, OLD.action, OLD.module, OLD.changed_fields);
END;

CREATE TRIGGER IF NOT EXISTS operation_logs_au AFTER UPDATE ON operation_logs BEGIN
  INSERT INTO logs_search(logs_search, rowid, user_name, target_title, action, module, changed_fields)
  VALUES('delete', OLD.rowid, OLD.user_name, OLD.target_title, OLD.action, OLD.module, OLD.changed_fields);
  INSERT INTO logs_search(rowid, user_name, target_title, action, module, changed_fields)
  VALUES (NEW.rowid, NEW.user_name, NEW.target_title, NEW.action, NEW.module, NEW.changed_fields);
END;

