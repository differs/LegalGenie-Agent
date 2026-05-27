#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

if ! command -v docker >/dev/null 2>&1; then
  echo "docker not found. Install Docker Desktop / docker engine first." >&2
  exit 1
fi

echo "Starting server via docker compose..."
docker compose up --build -d
echo "OK"
echo "Logs: docker compose logs -f server"

