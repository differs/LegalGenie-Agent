# Docker Runbook (Local)

This runs the backend server in Docker with local volumes for SQLite and `storage/`.

## 1) Prep `.env` (Recommended)

Docker Compose will run with built-in defaults, but you will usually want a local `.env`
to override secrets and limits.

Create `.env` from `.env.example` and adjust values as needed (Compose reads `.env` automatically).

Important defaults:

- `JWT_SECRET`: change for anything beyond local dev
- `CORS_ORIGINS`: keep `*` only in development

## 2) (Optional) Download Model / OCR Language Data

These are mounted into the container via `./data`.

```bash
./scripts/download_whisper_model.sh tiny
./scripts/download_tesseract_lang.sh chi_sim
```

## 3) Start

```bash
docker compose up --build
```

Health check:

```bash
curl -s http://127.0.0.1:8000/api/v1/health
```

## 4) Notes

- File parsing for PDF/OCR/audio is available because the image installs Poppler/Tesseract/FFmpeg.
- Audio ASR still needs the whisper model file on disk (mounted at `WHISPER_MODEL_PATH`).
- Data persists on the host in `./data` and `./storage`.
