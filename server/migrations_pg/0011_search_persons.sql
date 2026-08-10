-- Person search mirror (trigram).
CREATE TABLE IF NOT EXISTS search_persons (
    rowid        TEXT PRIMARY KEY,
    name         TEXT NOT NULL DEFAULT '',
    phone        TEXT NOT NULL DEFAULT '',
    email        TEXT NOT NULL DEFAULT '',
    organization TEXT NOT NULL DEFAULT '',
    position     TEXT NOT NULL DEFAULT '',
    notes        TEXT NOT NULL DEFAULT '',
    searchable   TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS search_persons_trgm_idx
    ON search_persons USING GIN (searchable gin_trgm_ops);

CREATE OR REPLACE FUNCTION fn_search_persons_sync()
RETURNS trigger AS $$
BEGIN
  IF TG_OP = 'DELETE' THEN
    DELETE FROM search_persons WHERE rowid = OLD.id;
    RETURN OLD;
  END IF;
  INSERT INTO search_persons (rowid, name, phone, email, organization, position, notes, searchable)
  VALUES (
    NEW.id, NEW.name, COALESCE(NEW.phone, ''), COALESCE(NEW.email, ''),
    COALESCE(NEW.organization, ''), COALESCE(NEW.position, ''), COALESCE(NEW.notes, ''),
    COALESCE(NEW.name, '') || ' ' || COALESCE(NEW.phone, '') || ' ' || COALESCE(NEW.email, '')
      || ' ' || COALESCE(NEW.organization, '') || ' ' || COALESCE(NEW.position, '')
      || ' ' || COALESCE(NEW.notes, '')
  )
  ON CONFLICT (rowid) DO UPDATE SET
    name = EXCLUDED.name,
    phone = EXCLUDED.phone,
    email = EXCLUDED.email,
    organization = EXCLUDED.organization,
    position = EXCLUDED.position,
    notes = EXCLUDED.notes,
    searchable = EXCLUDED.searchable;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS persons_search_sync ON persons;
CREATE TRIGGER persons_search_sync
AFTER INSERT OR UPDATE OR DELETE ON persons
FOR EACH ROW EXECUTE FUNCTION fn_search_persons_sync();
