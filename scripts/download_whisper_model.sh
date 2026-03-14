#!/usr/bin/env bash
set -euo pipefail

MODEL_NAME="${1:-tiny}"
OUT_PATH="${2:-./data/models/whisper/ggml-${MODEL_NAME}.bin}"

mkdir -p "$(dirname "$OUT_PATH")"

URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-${MODEL_NAME}.bin"

echo "Downloading whisper.cpp model: ${MODEL_NAME}"
echo "From: ${URL}"
echo "To:   ${OUT_PATH}"

curl -L --fail -o "${OUT_PATH}.tmp" "${URL}"
mv "${OUT_PATH}.tmp" "${OUT_PATH}"

echo "OK"

