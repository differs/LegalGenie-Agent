# API Acceptance Smoke (curl)

This is a **scripted** acceptance checklist for the backend API that matches the current product scope:

- Timeline (nodes + tag filter + evidence link/unlink)
- Persons (dedupe suggestions + case-local merge + case-local relationships)
- Permissions (viewer read-only for mutations)
- Logs history endpoint
- Evidence list export

## Prerequisites

- Server is running on `http://127.0.0.1:8000` (or set `API_BASE`)
- Tools: `curl`, `jq`

## Run (Recommended)

```bash
API_BASE=http://127.0.0.1:8000 ./scripts/acceptance_api_smoke.sh
```

Expected result: the script ends with `ALL PASS: acceptance_api_smoke (...)`.

## Run (One-Shot, Starts Server Automatically)

This will start a temporary server on a random local port with an isolated temp SQLite DB + storage,
run the same acceptance checks, then shut the server down:

```bash
./scripts/acceptance_one_shot.sh
```

## What It Verifies (HTTP Expectations)

This script executes real `curl` requests and verifies:

- `GET /api/v1/health` -> `200`
- Register 3 users -> `200`
- Create case (owner) -> `200`
- Add member/viewer (owner) -> `200`
- `GET /cases/:id/members/me` -> `200` and role matches
- Timeline:
  - member creates nodes -> `200`
  - tags filter `tags=alpha,beta` returns only the tagged node -> `200`
  - viewer create node -> `403`
- Files:
  - member upload -> `200`
  - viewer upload -> `403`
  - link/unlink evidence to node -> `200`
  - parse txt evidence -> `200` then `parse_status != processing`
- Persons:
  - create 3 persons -> `200`
  - viewer dedupe -> `200` and contains `name:zhang san` group
  - create relationship -> `200`
  - `GET /cases/:id/persons/graph` returns nodes + edges reflecting relationships -> `200`
  - viewer create relationship -> `403`
  - viewer merge persons -> `403`
  - owner merge persons -> `200` and moves links/relationships (case-local)
- Logs:
  - `GET /api/v1/logs/:target_type/:target_id/history` -> `200` (person/node/person_relationship)
- Exports:
  - `GET /cases/:id/exports/evidence-list` -> `200`
