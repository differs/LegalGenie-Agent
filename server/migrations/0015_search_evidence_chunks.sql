-- Chunk-level bilingual full-text search for parsed evidence content.
CREATE VIRTUAL TABLE IF NOT EXISTS search_evidence_chunks USING fts5(
    source_text,
    translated_text,
    content='evidence_file_chunks',
    content_rowid='rowid',
    tokenize='unicode61'
);

CREATE TRIGGER IF NOT EXISTS evidence_file_chunks_ai
AFTER INSERT ON evidence_file_chunks BEGIN
  INSERT INTO search_evidence_chunks(rowid, source_text, translated_text)
  VALUES (NEW.rowid, NEW.source_text, NEW.translated_text);
END;

CREATE TRIGGER IF NOT EXISTS evidence_file_chunks_ad
AFTER DELETE ON evidence_file_chunks BEGIN
  INSERT INTO search_evidence_chunks(
      search_evidence_chunks,
      rowid,
      source_text,
      translated_text
  )
  VALUES ('delete', OLD.rowid, OLD.source_text, OLD.translated_text);
END;

CREATE TRIGGER IF NOT EXISTS evidence_file_chunks_au
AFTER UPDATE ON evidence_file_chunks BEGIN
  INSERT INTO search_evidence_chunks(
      search_evidence_chunks,
      rowid,
      source_text,
      translated_text
  )
  VALUES ('delete', OLD.rowid, OLD.source_text, OLD.translated_text);
  INSERT INTO search_evidence_chunks(rowid, source_text, translated_text)
  VALUES (NEW.rowid, NEW.source_text, NEW.translated_text);
END;

INSERT INTO search_evidence_chunks(search_evidence_chunks) VALUES('rebuild');
