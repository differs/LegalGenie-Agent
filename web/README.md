# LegalGenie Agent Web

Standalone Web frontend for LegalGenie Agent — a chat-first legal workbench.

## Stack

- Vite 8
- React 19
- Tailwind CSS 4
- Bun
- react-router-dom (hash routing)

## Development

```bash
bun install
bun dev
```

## Build

```bash
bun run build      # tsc -b && vite build
bun run lint       # eslint
```

Output: `dist/index.html` (workbench) + `dist/overview.html` (static overview page).

## API Connection

The frontend connects to the backend through `VITE_API_BASE_URL` (default
`http://127.0.0.1:8001/api/v1`).

## Interaction Model

- **Chat-first workbench**: the left rail switches between Chat / Timeline /
  Evidence / Persons / Search / Exports / Logs. The Chat view parses natural
  language intents against the current case.
- **Preview -> Confirm**: every write intent (create node, export, person
  merge, ...) lands in the right-hand "待确认动作" queue as an approval card.
  Nothing is written until the user confirms.
- **Role-aware**: `viewer` role is read-only in the UI and the agent refuses
  write intents.
- **Session flow**: login/register → case switcher → per-case chat history
  (session-scoped), automatic token refresh on boot.

## Layout

```
src/
  App.tsx            # hash router + auth provider
  auth/AuthPage.tsx  # login / register
  lib/               # api client, types, local agent engine, hooks
  workspace/         # workbench: rail, chat+cards, timeline, evidence,
                     # persons(+dedupe/merge/relationships), search, exports, logs
  site/              # static overview page shell
  entries/           # vite entry points (home -> App, overview -> site)
```
