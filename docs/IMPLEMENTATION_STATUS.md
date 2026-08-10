# LegalGenie Agent Implementation Status (Docs vs Code)

Last updated: 2026-03-18

This document is the single source of truth for "what is implemented now" in this repo,
and highlights the gaps between requirement documents and current code.

Decision log for resolving spec mismatches (implement vs re-scope):
`docs/GOAL_REVIEW.md`.

## Product Maturity Level (5-Level Rubric)

We use an internal 5-level maturity rubric for "function design completeness":

- L1 Idea: feature list / pitch only, no clear scope boundaries.
- L2 PRD: clear MVP scope + user roles + primary flows, but no executable spec.
- L3 Buildable: API + DB + key flows specified AND a runnable implementation exists for MVP.
- L4 Deliverable: docs <-> implementation aligned, permission boundaries enforced, deployment/runbook exists, key UX flows wired.
- L5 Scalable: multi-tenant/enterprise hardening, observability, performance budgets, operations SOPs, and regression test depth.

Current assessment: **L4 (Deliverable)**.

## What Is Implemented (Back-End)

Backend stack: Axum + Tokio + SQLx (SQLite), server-side rendered API under `/api/v1`.

Implemented modules and representative endpoints (see README for the full list):

- Auth (JWT access + refresh)
  - `POST /api/v1/auth/register`
  - `POST /api/v1/auth/login`
  - `POST /api/v1/auth/refresh`
  - `POST /api/v1/auth/logout` (stateless; audit only)
  - `GET /api/v1/auth/me`
  - `PUT /api/v1/auth/password`
- Cases (CRUD + membership)
  - `GET/POST /api/v1/cases`
  - `GET/PUT/DELETE /api/v1/cases/:id`
  - `GET/POST /api/v1/cases/:id/members`
  - `DELETE /api/v1/cases/:id/members/:user_id`
- Evidence files (upload, list, preview/download, parse)
  - `POST /api/v1/cases/:case_id/files` (multipart)
  - `GET /api/v1/cases/:case_id/files`
  - `GET/DELETE /api/v1/files/:id`
  - `GET /api/v1/files/:id/download`
  - `GET /api/v1/files/:id/preview`
  - `POST /api/v1/files/:id/parse` (async)
  - `GET /api/v1/files/:id/parsed` (download parsed JSON artifact)
- Timeline nodes (CRUD + move + link/unlink evidence)
  - `GET/POST /api/v1/cases/:case_id/timeline/nodes`
  - `GET/PUT/DELETE /api/v1/timeline/nodes/:id`
  - `POST /api/v1/timeline/nodes/:id/move`
  - `POST /api/v1/timeline/nodes/:id/evidence`
  - `DELETE /api/v1/timeline/nodes/:id/evidence/:link_id`
- Search (FTS5)
  - `GET /api/v1/search` (cases/evidence/nodes)
  - `GET /api/v1/search/cases|evidence|nodes`
  - `GET /api/v1/search/suggestions`
  - `GET/DELETE /api/v1/search/history`
- Persons (minimal)
  - `GET/POST /api/v1/cases/:case_id/persons`
  - `GET /api/v1/cases/:case_id/persons/graph` (nodes + case-local relationship edges)
  - `GET /api/v1/cases/:case_id/persons/dedupe` (case-local dedupe suggestions)
  - `POST /api/v1/cases/:case_id/persons/merge` (case-local merge)
  - `GET/POST /api/v1/cases/:case_id/persons/relationships`
  - `DELETE /api/v1/cases/:case_id/persons/relationships/:id`
  - `GET/PUT/DELETE /api/v1/persons/:id`
  - `POST /api/v1/persons/:id/cases` (link person to case)
- Audit logs
  - `GET /api/v1/logs` (filters + access control)
  - `GET /api/v1/logs/export` (csv/xlsx)
  - `GET /api/v1/logs/:target_type/:target_id/history`
- Exports
  - Evidence list: `GET /api/v1/cases/:case_id/exports/evidence-list` (xlsx)
  - Timeline image: `GET /api/v1/cases/:case_id/exports/timeline?format=png`
  - Timeline report: `GET /api/v1/cases/:case_id/exports/timeline-report?format=pdf|html`
  - Export history: `GET /api/v1/cases/:case_id/exports/history`
  - Download export: `GET /api/v1/cases/:case_id/exports/:export_id/download`

## What Is Implemented (Front-End)

The Dioxus client has been removed; the Web frontend (`web/`, Vite + React + Tailwind CSS + Bun) is the only client.

The previous Dioxus implementation (now removed) had wired:

- Tabs: Cases, Files, Timeline, Persons, Search, Exports, Logs
- Timeline workspace: drag/zoom canvas (cursor-centered wheel zoom), drag-to-move with execute-mode gating + undo, selection + context lock, viewport culling + load-more up to 500 nodes, chat tools (nodes + evidence links + files + persons + search + exports) with preview -> confirm for writes
- Persons: list/create, detail (notes), link person to another case (minimal)
- Persons (case-local): dedupe suggestions + merge + relationships list/create/delete + lightweight SVG relationship graph view (chat-first)
- Search: multi-object search + suggestions + history

The React Web frontend currently provides a landing/overview page plus a workbench concept page; the API-facing workspace pages are to be (re)built on top of these.

Still missing from the requirement docs' "core product UX":

- Timeline advanced canvas behaviors (grouping, richer conflict hints, etc.)
- Person split flows; advanced relationship graph visualization/editing (pan/zoom/drag + graph-based CRUD)

## Key Gaps (Docs vs Code)

These gaps are what keep us from **L5 (Scalable)** and from the original long-term requirement docs.

### 1) Parsing Tech Route Mismatch

Some requirement docs describe: Python FFI + PyMuPDF + PaddleOCR + FunASR.

Current code implements a CLI + local tools pipeline:

- PDF: `pdftotext` + `pdfinfo`
- Images: `tesseract` (with optional `chi_sim` language data)
- Audio: `ffmpeg` + `ffprobe` + `whisper-rs` (whisper.cpp model file)

Status: README + runbooks are updated to match the actual implementation. Remaining work is to align older requirement docs (if we still want them as a source of truth).

### 2) Permission Model: Needs Hardening

Design docs propose RBAC + case-level permissions (owner/member/viewer).

Current code enforces (server-side):

- Case access: owner OR any case member
- Case write access: owner/member only (viewer is read-only for mutations)
- Case owner: required for case delete and member management

Remaining:

- Define a single "permission matrix" (owner/member/viewer) and verify every mutation endpoint and UI action against it.
- Add more negative tests for role-based denial on key mutations (timeline update/delete/move/unlink; persons delete/link; exports history if restricted later).

### 3) Error Code Granularity

Docs define module-specific error codes (e.g. 400101 for invalid credentials).

Current code now supports module-specific `error_code` values and partially aligns with the docs, for example:

- Auth: invalid credentials `400101`, weak password `400105`, too many attempts `400104`
- Token: invalid token `401001`
- Cases: not found `410101`, no permission `410102`
- Files: not found `420101`, too large `420102`, type not allowed/mismatch `420103`
- Timeline: node not found `430101`
- Persons: not found `440101`

Action: continue aligning remaining endpoints and decide whether to standardize HTTP status semantics (e.g., 400 vs 401 for invalid credentials), while keeping backward compatibility for clients.

### 4) Persons: Advanced Requirements Not Implemented

Requirement docs include:

- Same-name disambiguation
- Merge/split with history
- One person with multiple contact methods
- Person-person relationship edges and graph visualization

Current code is a minimal `persons` + `person_case_links` implementation.

Now implemented (MVP, case-local):

- Dedupe suggestions (simple name/phone/email grouping)
- Merge within a case (moves/merges links + relationships inside the case)
- Person-person relationships within a case (CRUD)

Remaining:

- Split flows / identity disambiguation UX
- Rich relationship graph visualization + editing

### 5) Search Scope Not Fully Matching Docs

Docs include person search and an "all objects" unified search view.

Current code searches: cases/evidence/nodes/persons (use `object_types=person`).

Action: optional: add a dedicated `/search/persons` endpoint for symmetry and ensure scoring/highlighting is consistent across all object types.

## Next Steps (Step-by-Step)

1. (Done in this step) Create/maintain this implementation matrix and update README dependencies.
2. Enforce case roles: viewer read-only; tighten mutation endpoints.
3. Add a deployment/runbook that matches actual parsing dependencies (Poppler/Tesseract/FFmpeg/Whisper model).
4. Decide and implement error code strategy (docs-aligned or simplify docs).
5. Decide persons/search MVP boundary and implement: person search + optional dedupe/merge roadmap.
