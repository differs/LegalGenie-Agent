#!/usr/bin/env bash
set -euo pipefail

BACKUP_DIR="${1:-}"
if [[ -z "${BACKUP_DIR}" ]]; then
  echo "Usage: $0 <backup_dir>" >&2
  exit 2
fi

DB_FILE="${BACKUP_DIR%/}/legal_minds.db"
if [[ ! -f "${DB_FILE}" ]]; then
  echo "Missing DB in backup: ${DB_FILE}" >&2
  exit 1
fi

if ! command -v sqlite3 >/dev/null 2>&1; then
  echo "sqlite3 is required for verification. Install sqlite3 and re-run." >&2
  exit 1
fi

echo "Running integrity_check on: ${DB_FILE}"
sqlite3 "${DB_FILE}" "PRAGMA integrity_check;"
echo "OK"

