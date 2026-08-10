# LegalGenie Agent Development Plan

Last updated: 2026-03-18

This plan is intentionally pragmatic: ship a local-deployable v0.1 with a chat-first workflow,
where the Timeline canvas is the primary workspace and other modules can be handled via chat tools.

## Current Status

- Maturity: L4 (Deliverable) per `docs/IMPLEMENTATION_STATUS.md`.
- Primary UX direction:
  - Timeline canvas: highest-frequency direct manipulation (pan/zoom/drag-to-move).
  - Right-side chat panel: preview -> confirm for writes, plus tooling cards.

## Milestones

### W1 (Done): Case Roles + Read-Only Gating

- Server: `GET /api/v1/cases/:case_id/members/me` to identify role in case.
- Server: viewer is read-only for case mutations (tests included).
- Frontend: `case_role` fetched into `AppCtx`, Timeline/Files gate write actions.

### W2 (Done): Timeline Workspace Closure

Goal: complete Timeline work without relying on the legacy list/debug UI.

- Canvas:
  - wheel zoom
  - drag background to pan
  - drag node to move date/order lane (execute-mode + permission gated)
  - undo for drag move
- Context:
  - always show Selected + Locked Focus (when lock enabled)
  - lock/unlock context for chat
- Chat panel:
  - tools: create node, focus node (edit/move/link/unlink/delete), exports
  - write actions: preview -> confirm in Execute mode
  - exports: evidence list xlsx, timeline png, report pdf/html
- Layout:
  - right panel resizable width (desktop), stacked on small screens

### W3 (Done): Expand Chat-First Tools Beyond Timeline

Goal: keep user in the Timeline workspace and use chat tools for most other operations.

- Added chat tools in Timeline workspace for:
  - Files: pick/upload (preview -> confirm), parse/re-parse (preview -> confirm), download original/parsed, show parse errors
  - Search: filters + suggestions/history, open results (node opens and auto-adjusts date range)
  - Persons: list/create (preview -> confirm), edit notes (preview -> confirm), link to another case (preview -> confirm), delete (preview -> confirm)
- Backend support:
  - `GET /api/v1/timeline/nodes/:id` to open a node from Search reliably.

### W4 (Planned): Hardening Toward L5

- Publish a single permission matrix (owner/member/viewer) and cover it with negative tests. (Done)
- Continue error_code alignment and standardize client-visible error handling.
- Add a scripted acceptance checklist (curl) for API-level regression. (Done)
- Validate Docker runbook by actually building/running the image in CI (optional).
- Performance/UX: handle >200 timeline nodes gracefully. (Done: canvas load-more up to 500 + viewport culling)

### W5 (Done): Runbooks + Backup/Restore + CI

- Runbooks:
  - `docs/RUNBOOK_LOCAL.md`
  - `docs/RUNBOOK_DOCKER.md`
- Backup/restore scripts:
  - `scripts/backup_local.sh`
  - `scripts/restore_backup.sh`
- CI:
  - `.github/workflows/ci.yml` runs server tests
- Acceptance:
  - `scripts/acceptance_api_smoke.sh`
  - `docs/ACCEPTANCE_API_SMOKE.md`

### W6 (Done): Persons (Case-Local) Enhancements

- Dedupe suggestions (name/phone/email grouping)
- Case-local merge (moves/merges links + relationships within current case only)
- Case-local person-person relationships (CRUD + minimal list UI)

## Definition Of Done (v0.1)

- Local runbook works end-to-end: register/login -> create case -> upload evidence -> parse -> create timeline nodes -> link/unlink evidence -> export -> audit logs.
- Viewer role is read-only both server-side (403) and UI-side (disabled or gated).
- Timeline workspace supports the primary interaction loop without opening legacy/debug UI.
