-- Full-text search (FTS5) index for persons.

CREATE VIRTUAL TABLE IF NOT EXISTS search_persons USING fts5(
    name,
    phone,
    email,
    organization,
    position,
    notes,
    content='persons',
    content_rowid='rowid',
    tokenize='unicode61'
);

CREATE TRIGGER IF NOT EXISTS persons_ai AFTER INSERT ON persons BEGIN
  INSERT INTO search_persons(rowid, name, phone, email, organization, position, notes)
  VALUES (NEW.rowid, NEW.name, NEW.phone, NEW.email, NEW.organization, NEW.position, NEW.notes);
END;

CREATE TRIGGER IF NOT EXISTS persons_ad AFTER DELETE ON persons BEGIN
  INSERT INTO search_persons(search_persons, rowid, name, phone, email, organization, position, notes)
  VALUES('delete', OLD.rowid, OLD.name, OLD.phone, OLD.email, OLD.organization, OLD.position, OLD.notes);
END;

CREATE TRIGGER IF NOT EXISTS persons_au AFTER UPDATE ON persons BEGIN
  INSERT INTO search_persons(search_persons, rowid, name, phone, email, organization, position, notes)
  VALUES('delete', OLD.rowid, OLD.name, OLD.phone, OLD.email, OLD.organization, OLD.position, OLD.notes);
  INSERT INTO search_persons(rowid, name, phone, email, organization, position, notes)
  VALUES (NEW.rowid, NEW.name, NEW.phone, NEW.email, NEW.organization, NEW.position, NEW.notes);
END;

-- Build indexes for any pre-existing data (safe no-op on empty DB).
INSERT INTO search_persons(search_persons) VALUES('rebuild');

