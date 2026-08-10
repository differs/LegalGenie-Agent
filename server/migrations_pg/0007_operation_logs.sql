-- Operation logs (audit trail) + trigram search mirror.
CREATE TABLE IF NOT EXISTS operation_logs (
    id             TEXT PRIMARY KEY,
    user_id        TEXT NOT NULL REFERENCES users(id),
    user_name      TEXT NOT NULL,
    case_id        TEXT,
    action         TEXT NOT NULL,
    module         TEXT NOT NULL,
    target_type    TEXT NOT NULL,
    target_id      TEXT,
    target_title   TEXT,
    old_value      TEXT, -- JSON string
    new_value      TEXT, -- JSON string
    changed_fields TEXT,
    ip_address     TEXT,
    user_agent     TEXT,
    request_id     TEXT,
    created_at     TEXT NOT NULL DEFAULT utc_text()
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

-- Log search mirror (keyword search over user/target/action fields).
CREATE TABLE IF NOT EXISTS logs_search (
    rowid       TEXT PRIMARY KEY,
    user_name   TEXT NOT NULL DEFAULT '',
    target_title TEXT NOT NULL DEFAULT '',
    action      TEXT NOT NULL DEFAULT '',
    module      TEXT NOT NULL DEFAULT '',
    changed_fields TEXT NOT NULL DEFAULT '',
    searchable  TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS logs_search_trgm_idx
    ON logs_search USING GIN (searchable gin_trgm_ops);

CREATE OR REPLACE FUNCTION fn_logs_search_sync()
RETURNS trigger AS $$
BEGIN
  IF TG_OP = 'DELETE' THEN
    DELETE FROM logs_search WHERE rowid = OLD.id;
    RETURN OLD;
  END IF;
  INSERT INTO logs_search (rowid, user_name, target_title, action, module, changed_fields, searchable)
  VALUES (
    NEW.id, NEW.user_name, COALESCE(NEW.target_title, ''), NEW.action, NEW.module,
    COALESCE(NEW.changed_fields, ''),
    COALESCE(NEW.user_name, '') || ' ' || COALESCE(NEW.target_title, '') || ' ' || NEW.action
      || ' ' || NEW.module || ' ' || COALESCE(NEW.changed_fields, '')
  )
  ON CONFLICT (rowid) DO UPDATE SET
    user_name = EXCLUDED.user_name,
    target_title = EXCLUDED.target_title,
    action = EXCLUDED.action,
    module = EXCLUDED.module,
    changed_fields = EXCLUDED.changed_fields,
    searchable = EXCLUDED.searchable;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS operation_logs_search_sync ON operation_logs;
CREATE TRIGGER operation_logs_search_sync
AFTER INSERT OR UPDATE OR DELETE ON operation_logs
FOR EACH ROW EXECUTE FUNCTION fn_logs_search_sync();
