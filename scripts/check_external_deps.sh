#!/usr/bin/env bash
set -euo pipefail

fail=0

have_cmd() {
  command -v "$1" >/dev/null 2>&1
}

check_cmd() {
  local cmd="$1"
  local label="$2"
  if have_cmd "$cmd"; then
    echo "OK   $cmd ($label)"
  else
    echo "MISS $cmd ($label)"
    fail=1
  fi
}

warn_file() {
  local path="$1"
  local label="$2"
  if [[ -f "$path" ]]; then
    echo "OK   file $path ($label)"
  else
    echo "WARN file $path ($label) not found"
  fi
}

echo "Checking external dependencies..."

# PDF parsing (Poppler)
check_cmd "pdftotext" "PDF parsing"
check_cmd "pdfinfo" "PDF page count"

# OCR
check_cmd "tesseract" "OCR (images)"

# Audio parsing
check_cmd "ffmpeg" "Audio conversion"
check_cmd "ffprobe" "Audio metadata"

echo ""
echo "Checking optional data files..."

TESSDATA_DIR="${TESSDATA_DIR:-./data/tessdata}"
warn_file "${TESSDATA_DIR}/chi_sim.traineddata" "Tesseract Chinese language data"

WHISPER_MODEL_PATH="${WHISPER_MODEL_PATH:-./data/models/whisper/ggml-tiny.bin}"
warn_file "${WHISPER_MODEL_PATH}" "whisper.cpp model"

echo ""
if [[ "$fail" -eq 0 ]]; then
  echo "All required commands are present."
  exit 0
fi

cat <<'EOF'
One or more commands are missing.

Without these tools the server can still run, but file parsing will fail for those file types.
See docs/RUNBOOK_LOCAL.md for installation commands.
EOF

exit 1

