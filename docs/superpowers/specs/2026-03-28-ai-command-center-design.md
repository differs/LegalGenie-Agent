# AI Command Center Redesign

## Goal

Move the frontend away from a developer-style utility shell and into a single-case, AI-led legal workspace.

## Core Decisions

- The product is a `single-case AI cockpit`, not a dashboard and not a generic chat app.
- AI is the `primary driver`.
- Low-risk actions may auto-execute.
- Write actions should default to `review required`.
- Destructive or identity-altering actions stay `guarded`.

## Layout

### Logged-out state

- Replace the mixed top bar with a dedicated authentication landing page.
- Keep API base and token entry available, but move them into a separate connection card.
- Present the product promise first: AI brief, suggested actions, execution boundaries.

### Logged-in state

- Use a left navigation rail for primary workspaces:
  - Brief
  - Cases
  - Timeline
  - Evidence
  - Persons
  - Search
  - Exports
  - Logs
- Use a top case bar for current workspace context and session controls.
- Use a command deck above the page body:
  - AI brief
  - Suggested action queue
  - Current context card

## Execution Model

- `Auto`: safe, reversible navigation and summarization flows.
- `Review`: content-changing actions such as node creation or export generation.
- `Guarded`: identity merges, destructive changes, permission-sensitive work.

## UX Rules

- The first screen should explain what AI is ready to do.
- Connection controls should never dominate the workspace.
- The selected case remains the main unit of context.
- Auditability should be visible in wording and action labels.

## Implementation Scope

- Introduce a tested shell state model for briefs and action queues.
- Rebuild the outer shell in `frontend/src/main.rs`.
- Preserve existing page internals for timeline, files, persons, search, exports, and logs.
