# LegalMinds

This repo currently contains requirement/quotation documents plus a Rust code scaffold.

## Stack (Chosen)

- Frontend: Dioxus (web + desktop)
- Backend: Axum (Tokio)
- DB: SQLite (SQLx)

## Quick Start (Local)

1. Install Rust (stable) via rustup and ensure `wasm32-unknown-unknown` target is available.
2. Copy `.env.example` to `.env` and adjust if needed.
3. Run the server:

```bash
cargo run -p legalminds-server
```

4. Run the desktop app (Windows/Mac/Linux):

```bash
cargo run -p legalminds-frontend
```

5. Run the web app (requires `dioxus-cli`):

```bash
cargo install dioxus-cli
cd frontend
dx serve
```

Server health check: `GET /api/v1/health`

## API (Current)

- Auth: `POST /api/v1/auth/register`, `POST /api/v1/auth/login`, `POST /api/v1/auth/refresh`, `POST /api/v1/auth/logout`, `GET /api/v1/auth/me`, `PUT /api/v1/auth/password`
- Cases: `GET /api/v1/cases`, `POST /api/v1/cases`, `GET /api/v1/cases/:id`, `PUT /api/v1/cases/:id`, `DELETE /api/v1/cases/:id`
- Case Members: `GET /api/v1/cases/:id/members`, `POST /api/v1/cases/:id/members`, `DELETE /api/v1/cases/:id/members/:user_id`
- Files: `POST /api/v1/cases/:case_id/files` (multipart), `GET /api/v1/cases/:case_id/files`, `GET /api/v1/files/:id`, `GET /api/v1/files/:id/download`, `DELETE /api/v1/files/:id`
- File Preview: `GET /api/v1/files/:id/preview`, `POST /api/v1/files/:id/parse`
- Search: `GET /api/v1/search`, `GET /api/v1/search/cases`, `GET /api/v1/search/evidence`, `GET /api/v1/search/nodes`, `GET /api/v1/search/suggestions`, `GET /api/v1/search/history`, `DELETE /api/v1/search/history`
- Timeline: `GET /api/v1/cases/:case_id/timeline/nodes`, `POST /api/v1/cases/:case_id/timeline/nodes`, `PUT /api/v1/timeline/nodes/:id`, `DELETE /api/v1/timeline/nodes/:id`, `POST /api/v1/timeline/nodes/:id/move`, `POST /api/v1/timeline/nodes/:id/evidence`, `DELETE /api/v1/timeline/nodes/:id/evidence/:link_id`
- Logs: `GET /api/v1/logs`, `GET /api/v1/logs/:target_type/:target_id/history`
- Persons: `GET /api/v1/cases/:case_id/persons`, `POST /api/v1/cases/:case_id/persons`, `GET /api/v1/cases/:case_id/persons/graph`, `GET /api/v1/persons/:id`, `PUT /api/v1/persons/:id`, `DELETE /api/v1/persons/:id`, `POST /api/v1/persons/:id/cases`
- Exports: `GET /api/v1/cases/:case_id/exports/evidence-list`, `GET /api/v1/cases/:case_id/exports/history`, `GET /api/v1/cases/:case_id/exports/:export_id/download`, `GET /api/v1/cases/:case_id/exports/timeline`
