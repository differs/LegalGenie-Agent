-- Parse status tracking for evidence files.
ALTER TABLE evidence_files ADD COLUMN parse_status TEXT NOT NULL DEFAULT 'pending';
ALTER TABLE evidence_files ADD COLUMN parse_error TEXT;
ALTER TABLE evidence_files ADD COLUMN parsed_at TEXT;

UPDATE evidence_files
SET parse_status = CASE
    WHEN parsed_text IS NOT NULL AND length(parsed_text) > 0 THEN 'done'
    ELSE 'pending'
END
WHERE parse_status = 'pending';
