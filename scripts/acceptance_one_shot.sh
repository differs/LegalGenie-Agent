#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

require_cmd() {
  local cmd="$1"
  if ! command -v "${cmd}" >/dev/null 2>&1; then
    echo "Missing dependency: ${cmd}" >&2
    exit 2
  fi
}

require_cmd cargo
require_cmd curl
require_cmd jq
require_cmd python3

if [[ -n "${API_BASE:-}" ]]; then
  echo "Using existing server: ${API_BASE}"
  API_BASE="${API_BASE}" ./scripts/acceptance_api_smoke.sh
  exit 0
fi

PORT="$(
  python3 - <<'PY'
import socket
s = socket.socket()
s.bind(("127.0.0.1", 0))
print(s.getsockname()[1])
s.close()
PY
)"

TMP_DIR="$(mktemp -d)"
cleanup() { rm -rf "${TMP_DIR}"; }
trap cleanup EXIT

LOG_FILE="${TMP_DIR}/server.log"

export APP_ENV="${APP_ENV:-development}"
export SERVER_HOST="127.0.0.1"
export SERVER_PORT="${PORT}"
DB_FILE="${TMP_DIR}/legal_minds.db"
export DATABASE_URL="sqlite:///${DB_FILE#/}"
export STORAGE_PATH="${TMP_DIR}/storage"
export TEMP_PATH="${TMP_DIR}/storage/temp"
export TESSDATA_DIR="${TMP_DIR}/tessdata"
export WHISPER_MODEL_PATH="${TMP_DIR}/whisper.bin"
export JWT_SECRET="${JWT_SECRET:-test-secret-please-change-32-chars-min}"
export CORS_ORIGINS="${CORS_ORIGINS:-*}"

echo "Building server..."
cargo build -p legalminds-server >/dev/null

echo "Starting server on http://127.0.0.1:${PORT} ..."
mkdir -p "${STORAGE_PATH}" "${TEMP_PATH}" "${TESSDATA_DIR}"
touch "${DB_FILE}"
"${ROOT}/target/debug/legalminds-server" >"${LOG_FILE}" 2>&1 &
SERVER_PID="$!"

stop_server() {
  if kill -0 "${SERVER_PID}" >/dev/null 2>&1; then
    kill "${SERVER_PID}" >/dev/null 2>&1 || true
    wait "${SERVER_PID}" >/dev/null 2>&1 || true
  fi
}
trap stop_server EXIT

READY="0"
for _ in $(seq 1 80); do
  code="$(curl -s -o /dev/null -w "%{http_code}" "http://127.0.0.1:${PORT}/api/v1/health" || true)"
  if [[ "${code}" == "200" ]]; then
    READY="1"
    break
  fi
  sleep 0.15
done

if [[ "${READY}" != "1" ]]; then
  echo "Server did not become ready. Log:" >&2
  tail -n 120 "${LOG_FILE}" >&2 || true
  exit 1
fi

echo "Running acceptance API smoke..."
API_BASE="http://127.0.0.1:${PORT}" ./scripts/acceptance_api_smoke.sh
