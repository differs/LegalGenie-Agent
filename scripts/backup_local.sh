#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TS="$(date +%Y%m%d_%H%M%S)"

OUT_DIR="${1:-"${ROOT}/backups"}"
DB_PATH="${DB_PATH:-"${ROOT}/data/legal_minds.db"}"
STORAGE_PATH="${STORAGE_PATH:-"${ROOT}/storage"}"

BACKUP_DIR="${OUT_DIR}/${TS}"
mkdir -p "${BACKUP_DIR}"

if [[ ! -f "${DB_PATH}" ]]; then
  echo "DB not found: ${DB_PATH}" >&2
  exit 1
fi

if [[ ! -d "${STORAGE_PATH}" ]]; then
  echo "Storage dir not found: ${STORAGE_PATH}" >&2
  exit 1
fi

echo "Backing up DB: ${DB_PATH}"
if command -v sqlite3 >/dev/null 2>&1; then
  sqlite3 "${DB_PATH}" ".backup '${BACKUP_DIR}/legal_minds.db'"
else
  # Best-effort fallback. For maximum safety, install sqlite3 and use `.backup`.
  cp "${DB_PATH}" "${BACKUP_DIR}/legal_minds.db"
fi

echo "Backing up storage: ${STORAGE_PATH}"
tar -czf "${BACKUP_DIR}/storage.tar.gz" -C "$(dirname "${STORAGE_PATH}")" "$(basename "${STORAGE_PATH}")"

cat > "${BACKUP_DIR}/manifest.txt" <<EOF
created_at=${TS}
db_path=${DB_PATH}
storage_path=${STORAGE_PATH}
EOF

echo "OK: ${BACKUP_DIR}"

