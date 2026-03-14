-- Add template information to export records (for auditability).

ALTER TABLE export_records ADD COLUMN template_name TEXT;

