-- Audit hardening (P4): operation_logs is append-only; historical cleanup is
-- an explicit retention job, never in-place mutation.
CREATE OR REPLACE FUNCTION prevent_operation_log_mutation()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'operation_logs is append-only; mutation denied';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS operation_logs_no_update ON operation_logs;
CREATE TRIGGER operation_logs_no_update
    BEFORE UPDATE OR DELETE ON operation_logs
    FOR EACH ROW EXECUTE FUNCTION prevent_operation_log_mutation();

-- Retention: purge logs older than LOG_RETENTION_DAYS (default 90) when the
-- server boots and then daily.
CREATE OR REPLACE FUNCTION purge_operation_logs(days INTEGER)
RETURNS BIGINT AS $$
DECLARE
    cutoff TEXT;
    removed BIGINT;
BEGIN
    cutoff := to_char(now() AT TIME ZONE 'UTC' - make_interval(days => days),
                      'YYYY-MM-DD HH24:MI:SS');
    DELETE FROM operation_logs WHERE created_at < cutoff;
    GET DIAGNOSTICS removed = ROW_COUNT;
    RETURN removed;
END;
$$ LANGUAGE plpgsql;
