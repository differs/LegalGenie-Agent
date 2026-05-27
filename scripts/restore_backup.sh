#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<EOF
Usage: $0 <backup_dir> [--force]

Restores a backup created by ./scripts/backup_local.sh into:
  - DB:      \${DB_PATH:-./data/legal_minds.db}
  - Storage: \${STORAGE_PATH:-./storage}

This is destructive. By default it refuses to overwrite existing data.
Re-run with --force (or FORCE=1) after stopping the server.
EOF
}

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

BACKUP_DIR="${1:-}"
if [[ -z "${BACKUP_DIR}" ]]; then
  usage
  exit 2
fi

FORCE_FLAG="${FORCE:-0}"
if [[ "${2:-}" == "--force" ]]; then
  FORCE_FLAG=1
fi

DB_PATH="${DB_PATH:-"${ROOT}/data/legal_minds.db"}"
STORAGE_PATH="${STORAGE_PATH:-"${ROOT}/storage"}"

BACKUP_DIR="${BACKUP_DIR%/}"
DB_FILE="${BACKUP_DIR}/legal_minds.db"
STORAGE_TAR="${BACKUP_DIR}/storage.tar.gz"

if [[ ! -f "${DB_FILE}" ]]; then
  echo "Missing DB in backup: ${DB_FILE}" >&2
  exit 1
fi
if [[ ! -f "${STORAGE_TAR}" ]]; then
  echo "Missing storage archive in backup: ${STORAGE_TAR}" >&2
  exit 1
fi

if [[ "${FORCE_FLAG}" != "1" ]]; then
  echo "Refusing to restore without --force (or FORCE=1)." >&2
  echo "This would overwrite:" >&2
  echo "  DB:      ${DB_PATH}" >&2
  echo "  Storage: ${STORAGE_PATH}" >&2
  exit 2
fi

TS="$(date +%Y%m%d_%H%M%S)"
PRE_DIR="${ROOT}/backups/pre_restore_${TS}"
mkdir -p "${PRE_DIR}"

echo "Safety backup (pre-restore): ${PRE_DIR}"
if [[ -f "${DB_PATH}" ]]; then
  cp "${DB_PATH}" "${PRE_DIR}/legal_minds.db"
fi
if [[ -d "${STORAGE_PATH}" ]]; then
  tar -czf "${PRE_DIR}/storage.tar.gz" -C "$(dirname "${STORAGE_PATH}")" "$(basename "${STORAGE_PATH}")"
fi

TMP="$(mktemp -d)"
cleanup() { rm -rf "${TMP}"; }
trap cleanup EXIT

echo "Restoring DB -> ${DB_PATH}"
mkdir -p "$(dirname "${DB_PATH}")"
cp "${DB_FILE}" "${DB_PATH}"

echo "Restoring storage -> ${STORAGE_PATH}"
mkdir -p "${TMP}/extract"
tar -xzf "${STORAGE_TAR}" -C "${TMP}/extract"

EXTRACTED_DIR="$(find "${TMP}/extract" -mindepth 1 -maxdepth 1 -type d | head -n 1 || true)"
if [[ -z "${EXTRACTED_DIR}" ]]; then
  echo "Invalid storage archive (no top-level dir): ${STORAGE_TAR}" >&2
  exit 1
fi

rm -rf "${STORAGE_PATH}"
mkdir -p "$(dirname "${STORAGE_PATH}")"
mv "${EXTRACTED_DIR}" "${STORAGE_PATH}"

cat > "${PRE_DIR}/manifest.txt" <<EOF
created_at=${TS}
backup_dir=${BACKUP_DIR}
restored_db_path=${DB_PATH}
restored_storage_path=${STORAGE_PATH}
EOF

echo "OK"
echo "Pre-restore backup saved at: ${PRE_DIR}"

