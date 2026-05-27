# LegalGenie Agent Web

Standalone Web frontend for LegalGenie Agent.

## Stack

- Vite
- React 19
- Tailwind CSS 4
- Bun

## Development

```bash
bun install
bun dev
```

## Build

```bash
bun run build
```

## API Connection

The frontend connects to the existing backend API through `VITE_API_BASE_URL`.

Default:

```bash
VITE_API_BASE_URL=http://127.0.0.1:8001/api/v1
```

Currently connected routes:

- `GET /health`
- `POST /auth/login`
- `GET /cases`

If no authenticated session is available, the UI falls back to demo content for presentation.
