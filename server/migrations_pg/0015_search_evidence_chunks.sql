-- Chunk-level bilingual search mirror (trigram).
CREATE TABLE IF NOT EXISTS search_evidence_chunks (
    rowid                TEXT PRIMARY KEY,
    source_text          TEXT NOT NULL DEFAULT '',
    translated_text      TEXT NOT NULL DEFAULT '',
    searchable           TEXT NOT NULL DEFAULT '',
    searchable_source    TEXT NOT NULL DEFAULT '',
    searchable_translated TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS search_evidence_chunks_trgm_idx
    ON search_evidence_chunks USING GIN (searchable gin_trgm_ops);
CREATE INDEX IF NOT EXISTS search_evidence_chunks_trgm_source_idx
    ON search_evidence_chunks USING GIN (searchable_source gin_trgm_ops);
CREATE INDEX IF NOT EXISTS search_evidence_chunks_trgm_translated_idx
    ON search_evidence_chunks USING GIN (searchable_translated gin_trgm_ops);

CREATE OR REPLACE FUNCTION fn_search_evidence_chunks_sync()
RETURNS trigger AS $$
BEGIN
  IF TG_OP = 'DELETE' THEN
    DELETE FROM search_evidence_chunks WHERE rowid = OLD.id;
    RETURN OLD;
  END IF;
  INSERT INTO search_evidence_chunks (rowid, source_text, translated_text, searchable, searchable_source, searchable_translated)
  VALUES (
    NEW.id, NEW.source_text, COALESCE(NEW.translated_text, ''),
    COALESCE(NEW.source_text, '') || ' ' || COALESCE(NEW.translated_text, ''),
    COALESCE(NEW.source_text, ''), COALESCE(NEW.translated_text, '')
  )
  ON CONFLICT (rowid) DO UPDATE SET
    source_text = EXCLUDED.source_text,
    translated_text = EXCLUDED.translated_text,
    searchable = EXCLUDED.searchable,
    searchable_source = EXCLUDED.searchable_source,
    searchable_translated = EXCLUDED.searchable_translated;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS evidence_file_chunks_search_sync ON evidence_file_chunks;
CREATE TRIGGER evidence_file_chunks_search_sync
AFTER INSERT OR UPDATE OR DELETE ON evidence_file_chunks
FOR EACH ROW EXECUTE FUNCTION fn_search_evidence_chunks_sync();
