# Goal Review (Docs vs Implementation)

Last updated: 2026-03-18

This doc is a decision log to resolve "spec vs code" mismatches by choosing one of:

- Implement the spec (code work).
- Re-scope the goal (doc/goal update) while keeping a clear roadmap.

## Current Target (v0.1)

Deliver a single-tenant, local-deployable MVP that supports:

- Auth (register/login/refresh), basic user info.
- Case CRUD + members (owner/member/viewer), viewer is read-only for case mutations.
- Evidence file upload/list/download/preview + async parsing (best-effort based on local deps).
- Timeline nodes CRUD + move + link/unlink evidence.
- Persons (minimal CRUD + case links) and person search.
- Unified search (cases/evidence/nodes/persons) + suggestions + history.
- Audit logs + exports.

Non-goals for v0.1 (explicitly deferred):

- Advanced timeline canvas behaviors (grouping, conflict hints, richer filters/navigation).
- Person split / identity disambiguation and rich relationship graph visualization/editing.
- Enterprise RBAC (global roles like admin/host_lawyer) and tenant isolation.

## Mismatch Decisions

### Parsing Tech

- Requirement docs mention Python FFI (PyMuPDF/PaddleOCR/FunASR).
- Implementation uses local CLI tools + Rust libs:
  - Poppler (`pdftotext`, `pdfinfo`), optional Tesseract OCR, FFmpeg, whisper-rs model.

Decision: keep current pipeline for v0.1; update deployment docs/runbooks to match.

### Permission Model

- Requirement docs propose global RBAC + case-level permissions.
- Implementation enforces case roles (owner/member/viewer) and viewer read-only for mutations.

Decision: v0.1 uses case roles only; global RBAC deferred.

### Timeline UX

- Requirement docs target a canvas with drag/zoom and rich interactions.
- Implementation now provides a basic canvas with:
  - drag-to-move nodes across dates / order lanes
  - wheel zoom
  - drag background to pan
  - selection linked to the detail editor

Decision: v0.1 includes a practical canvas; advanced canvas features remain deferred.

### Persons Advanced Capabilities

- Requirement docs include dedupe/merge, multi-contact, relationship edges.
- Implementation includes case-local MVPs:
  - Dedupe suggestions (name/phone/email grouping)
  - Merge within a case (moves/merges links + relationships inside the case)
  - Person-person relationships within a case (CRUD)

Decision: v0.1 includes these capabilities in a **case-local** scope; global dedupe/merge/split and rich graph UX remain deferred.

## What To Implement Next (To Reach "Deliverable")

Status: mostly addressed (Docker runbook + backup scripts exist; timeline is a chat-first workspace).

Next (to harden toward L5):

- Define and enforce a single permission matrix (owner/member/viewer) and cover key denials with tests.
- Reduce "developer console" surface area by moving more flows into chat-first workspaces.
- Decide how "global" persons should be across cases and align UI + permissions accordingly.
