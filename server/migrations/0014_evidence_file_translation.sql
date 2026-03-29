-- File-level translation tracking fields.
ALTER TABLE evidence_files ADD COLUMN translation_status TEXT NOT NULL DEFAULT 'pending';
ALTER TABLE evidence_files ADD COLUMN translation_error TEXT;
ALTER TABLE evidence_files ADD COLUMN source_language TEXT;
ALTER TABLE evidence_files ADD COLUMN target_language TEXT;
ALTER TABLE evidence_files ADD COLUMN chunk_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE evidence_files ADD COLUMN translated_chunk_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE evidence_files ADD COLUMN failed_chunk_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE evidence_files ADD COLUMN translation_model TEXT;
ALTER TABLE evidence_files ADD COLUMN translation_provider TEXT;

-- Chunk-level translation source of truth.
CREATE TABLE IF NOT EXISTS evidence_file_chunks (
    id                  TEXT PRIMARY KEY,
    evidence_id         TEXT NOT NULL REFERENCES evidence_files(id) ON DELETE CASCADE,
    case_id             TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
    chunk_index         INTEGER NOT NULL,
    page_number         INTEGER NOT NULL DEFAULT 0,
    segment_number      INTEGER NOT NULL DEFAULT 0,
    chunk_kind          TEXT NOT NULL DEFAULT 'text',
    display_label       TEXT NOT NULL,
    source_text         TEXT NOT NULL,
    char_count          INTEGER NOT NULL DEFAULT 0,
    token_estimate      INTEGER NOT NULL DEFAULT 0,
    anchor_json         TEXT,
    source_text_hash    TEXT NOT NULL,
    source_language     TEXT,
    target_language     TEXT,
    translated_text     TEXT,
    translation_status  TEXT NOT NULL DEFAULT 'pending',
    retry_count         INTEGER NOT NULL DEFAULT 0,
    max_retries         INTEGER NOT NULL DEFAULT 3,
    last_attempt_at     DATETIME,
    next_retry_at       DATETIME,
    translation_error   TEXT,
    translated_at       DATETIME,
    created_at          DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at          DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_evidence_file_chunks_file_chunk
    ON evidence_file_chunks(evidence_id, chunk_index);
CREATE INDEX IF NOT EXISTS idx_evidence_file_chunks_case
    ON evidence_file_chunks(case_id);
CREATE INDEX IF NOT EXISTS idx_evidence_file_chunks_status
    ON evidence_file_chunks(translation_status);
CREATE INDEX IF NOT EXISTS idx_evidence_file_chunks_hash
    ON evidence_file_chunks(source_text_hash);

-- Legacy parsed artifacts must be reparsed before bilingual chunk translation can proceed.
UPDATE evidence_files
SET
    translation_status = 'failed',
    translation_error = 'reparse required for bilingual translation'
WHERE
    parse_status = 'done'
    AND parsed_text IS NOT NULL
    AND length(trim(parsed_text)) > 0
    AND NOT EXISTS (
        SELECT 1
        FROM evidence_file_chunks c
        WHERE c.evidence_id = evidence_files.id
    );
