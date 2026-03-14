#!/usr/bin/env bash
set -euo pipefail

LANG_CODE="${1:-chi_sim}"
TESSDATA_DIR="${2:-./data/tessdata}"

mkdir -p "${TESSDATA_DIR}"

# Use tessdata_fast for smaller downloads.
URL="https://github.com/tesseract-ocr/tessdata_fast/raw/main/${LANG_CODE}.traineddata"
OUT_PATH="${TESSDATA_DIR}/${LANG_CODE}.traineddata"

echo "Downloading tesseract language data: ${LANG_CODE}"
echo "From: ${URL}"
echo "To:   ${OUT_PATH}"

curl -L --fail -o "${OUT_PATH}.tmp" "${URL}"
mv "${OUT_PATH}.tmp" "${OUT_PATH}"

echo "OK"

