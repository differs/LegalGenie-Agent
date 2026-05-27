# Permissions Matrix (v0.1)

Last updated: 2026-03-18

This document is the source of truth for **what each case role can do**.
Backend enforcement is authoritative; UI gating must match it.

## Roles

- `owner`: case owner (the `cases.owner_id` user)
- `member`: case member with write access (`case_members.role_in_case='member'`)
- `viewer`: case member with read-only access (`case_members.role_in_case='viewer'`)

Note: Global roles (`roles`, `user_roles`) exist in schema but are **not used** for v0.1 authorization decisions.

## Actions (By Module)

Legend: ✅ allowed, ❌ denied.

### Cases

| Action | owner | member | viewer |
|---|---:|---:|---:|
| List cases | ✅ | ✅ | ✅ |
| View case detail | ✅ | ✅ | ✅ |
| Create case | ✅ | ✅ | ✅ |
| Update case | ✅ | ✅ | ❌ |
| Delete case | ✅ | ❌ | ❌ |
| List members | ✅ | ✅ | ✅ |
| Add/remove members | ✅ | ❌ | ❌ |

### Evidence Files

| Action | owner | member | viewer |
|---|---:|---:|---:|
| List case files | ✅ | ✅ | ✅ |
| Download/preview file | ✅ | ✅ | ✅ |
| Upload file | ✅ | ✅ | ❌ |
| Parse / re-parse file | ✅ | ✅ | ❌ |
| Delete file | ✅ | ✅ | ❌ |

### Timeline Nodes

| Action | owner | member | viewer |
|---|---:|---:|---:|
| List nodes (case) | ✅ | ✅ | ✅ |
| Read node detail | ✅ | ✅ | ✅ |
| Create node | ✅ | ✅ | ❌ |
| Update node | ✅ | ✅ | ❌ |
| Move/reorder node | ✅ | ✅ | ❌ |
| Delete node | ✅ | ✅ | ❌ |
| Link/unlink evidence | ✅ | ✅ | ❌ |

### Persons

| Action | owner | member | viewer |
|---|---:|---:|---:|
| List persons (case) | ✅ | ✅ | ✅ |
| Read person detail | ✅ | ✅ | ✅ |
| Create person (in case) | ✅ | ✅ | ❌ |
| Update person | ✅ | ✅ | ❌ |
| Delete person | ✅ | ✅ | ❌ |
| Link person to another case | ✅ | ✅ | ❌ |
| Dedupe suggestions (case) | ✅ | ✅ | ✅ |
| Merge persons (case-local) | ✅ | ✅ | ❌ |
| Person relationships (case-local) | ✅ | ✅ | ❌ |

### Search

| Action | owner | member | viewer |
|---|---:|---:|---:|
| Run search | ✅ | ✅ | ✅ |
| Suggestions/history | ✅ | ✅ | ✅ |
| Clear search history | ✅ | ✅ | ✅ |

### Exports / Logs

| Action | owner | member | viewer |
|---|---:|---:|---:|
| Generate exports | ✅ | ✅ | ✅ |
| Export history | ✅ | ✅ | ✅ |
| Download exports | ✅ | ✅ | ✅ |
| View logs | ✅ | ✅ | ✅ |
| Export logs | ✅ | ✅ | ✅ |
| Target history | ✅ | ✅ | ✅ |

## Implementation Notes

- Case access is enforced via `ensure_case_access`.
- Case write access is enforced via `ensure_case_write_access` (owner + member).
- Owner-only actions use `ensure_case_owner`.

