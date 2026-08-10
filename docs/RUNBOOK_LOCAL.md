# LegalGenie Agent Local Runbook (Dev)

This repo runs a Rust backend + a React Web frontend (`web/`, Vite + Bun + Tailwind CSS).

Some file parsing features depend on external command-line tools. If those tools are missing, the server will still start, but parsing for that file type will fail.

## 1) Prerequisites

- Rust toolchain (stable)
- Bun (`bun`) for the Web frontend
- SQLite (bundled via `sqlx` for Rust; no external service needed)
- Optional but recommended external tools:
  - Poppler utils: `pdftotext`, `pdfinfo` (PDF parsing)
  - Tesseract: `tesseract` (OCR for images)
  - FFmpeg: `ffmpeg`, `ffprobe` (audio conversion + duration)

Quick check (prints warnings if anything is missing):

```bash
./scripts/check_external_deps.sh
```

## 2) Install External Tools

Ubuntu/Debian:

```bash
sudo apt-get update
sudo apt-get install -y poppler-utils tesseract-ocr ffmpeg
```

macOS (Homebrew):

```bash
brew install poppler tesseract ffmpeg
```

Windows (Chocolatey):

```powershell
choco install -y poppler tesseract ffmpeg
```

Note: On Windows you may need to restart the terminal after installing so the commands are available on `PATH`.

## 3) Download Model / Language Data (Optional)

Whisper model (for audio ASR via `whisper-rs`):

```bash
./scripts/download_whisper_model.sh tiny
```

Tesseract Chinese language data (if you need OCR for Chinese scans):

```bash
./scripts/download_tesseract_lang.sh chi_sim
```

## 4) Configure Environment

Copy `.env.example` to `.env` and edit as needed:

```bash
cp .env.example .env
```

Common env vars:

- `SERVER_PORT` (set this to `8001` for the default frontend flow, or keep overriding it on the command line)
- `DATABASE_URL` (default `sqlite://data/legal_minds.db`)
- `STORAGE_PATH` (default `./storage`)
- `TESSDATA_DIR` (default `./data/tessdata`)
- `WHISPER_MODEL_PATH` (default `./data/models/whisper/ggml-tiny.bin`)
- `TRANSLATION_PROVIDER` (default `disabled`)
- `TRANSLATION_BASE_URL` (required when translation is enabled)
- `TRANSLATION_API_KEY` (optional for local deployments, required by most cloud providers)
- `TRANSLATION_MODEL` (required when translation is enabled)
- `TRANSLATION_TARGET_LANGUAGE` (default `zh-CN`)
- `TRANSLATION_MAX_CONCURRENCY` (default `2`)
- `TRANSLATION_CHUNK_SIZE_LIMIT` (default `2000`)

## 4.1) Translation Provider Modes

Runtime translation uses one backend abstraction:

- `TRANSLATION_PROVIDER=disabled`
  Translation worker stays off. Parsed files remain usable, but bilingual chunks are not produced.
- `TRANSLATION_PROVIDER=<any non-disabled name>`
  The server treats it as an OpenAI-compatible chat completions endpoint and requires:
  - `TRANSLATION_BASE_URL` pointing at the provider API root such as `https://.../v1`
  - `TRANSLATION_MODEL`
  - usually `TRANSLATION_API_KEY`

The server appends `/chat/completions` itself, so `TRANSLATION_BASE_URL` should not already include that suffix.

That means cloud and local deployment use the same code path:

- Cloud example:

```bash
export TRANSLATION_PROVIDER=openai
export TRANSLATION_BASE_URL=https://api.openai.com/v1
export TRANSLATION_API_KEY=...
export TRANSLATION_MODEL=<cloud-model-id>
```

- Local/OpenAI-compatible example:

```bash
export TRANSLATION_PROVIDER=local-llm
export TRANSLATION_BASE_URL=http://127.0.0.1:11434/v1
export TRANSLATION_API_KEY=dummy
export TRANSLATION_MODEL=<local-model-id>
```

Test note:

- `translation_smoke` uses an in-process fake provider for deterministic testing.
- That fake provider is test-only; you do not enable it through runtime env vars.

## 5) Run Backend

```bash
SERVER_PORT=8001 cargo run -p legalminds-server
```

Note: the Web frontend's default API base is `http://127.0.0.1:8001`. `.env.example` still starts at `SERVER_PORT=8000`, so for local dev you should either set `SERVER_PORT=8001` inside `.env` or always use the explicit `SERVER_PORT=8001 ...` command shown here.

Health check:

```bash
curl -s http://127.0.0.1:8001/api/v1/health
```

## 6) Run Frontend

```bash
cd web
bun install
bun dev
```

The frontend connects to `http://127.0.0.1:8001/api/v1` by default. Override with:

```bash
VITE_API_BASE_URL=http://127.0.0.1:8001/api/v1
```

## 6.1) Bilingual Evidence Notes

- New files parse into chunks first, then translate automatically in the background.
- Files page now polls for both `parse_status=processing` and `translation_status=processing`.
- Search defaults to `zh`, but the UI can switch to `source` or `bilingual`.
- Old files that only have `parsed_text` remain readable/searchable through compatibility fallback.
- `zh` search can still hit those old files, but the evidence hit will be marked with `source_fallback=true`.
- To get true bilingual chunk reading for old files, re-run parsing so the file can generate chunk rows.

## 7) Where Data Goes

By default:

- Uploaded files: `./storage/files/<case_id>/...`
- Parsed artifacts (JSON): `./storage/parsed/<case_id>/<evidence_id>.json`
- Exports: `./storage/exports/<case_id>/...`
- Temp files: `./storage/temp/...`

## 8) Backup / Restore (Local)

Create a local backup (SQLite + `storage/`) into `./backups/<timestamp>/`:

```bash
./scripts/backup_local.sh
```

Verify a backup DB file:

```bash
./scripts/verify_backup.sh backups/<timestamp>
```

Restore a backup (destructive, stop the server first):

```bash
./scripts/restore_backup.sh backups/<timestamp> --force
```

Optional env overrides:

- `DB_PATH` (default `./data/legal_minds.db`)
- `STORAGE_PATH` (default `./storage`)

## 9) API Acceptance Smoke (Optional)

If you want a scripted acceptance check of the current API scope:

```bash
./scripts/acceptance_api_smoke.sh
```

One-shot (starts a temporary local server + temp DB automatically):

```bash
./scripts/acceptance_one_shot.sh
```

See also: `docs/ACCEPTANCE_API_SMOKE.md`.
