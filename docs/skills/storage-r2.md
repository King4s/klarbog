---
name: klarbog-storage-r2
description: >-
  Object storage for Klarbog attachments (local DEV default, Cloudflare R2 EU
  scaffold). Use when attaching documents or wiring binary upload paths.
---

# Object storage (ADR-007)

## When to use

- Document attach should ensure storage is ready before writing `documents.json` metadata.
- Stage/prod may point at Cloudflare R2 (EU, fail-closed) instead of local `objects/`.

## Backend selection

Env `KLARBOG_STORAGE` (default `local`):

| Value | Behavior |
|-------|----------|
| `local` | Create `<company>/objects/` if missing; use [`LocalFsStore`] |
| `r2` | Validate `KLARBOG_R2_*` via [`R2Config::from_env`] (no local dir) |

```rust
use klarbog_storage::{klarbog_storage, KlarbogStorage};

let backend = klarbog_storage(company_path)?;
match backend {
    KlarbogStorage::Local(store) => { /* path_hint under objects/ */ }
    KlarbogStorage::R2(cfg) => { /* keys only in metadata for now */ }
}
```

## R2 env (never commit secrets)

- `KLARBOG_R2_ACCOUNT_ID`
- `KLARBOG_R2_ACCESS_KEY_ID`
- `KLARBOG_R2_SECRET_ACCESS_KEY`
- `KLARBOG_R2_BUCKET`
- Optional: `KLARBOG_R2_JURISDICTION` (default `eu` / WEUR)
- Override non-EU only with `KLARBOG_R2_ALLOW_NON_EU=1`

`R2Store` network I/O is still a scaffold (`NotImplementedInDev`); config validation is live.

## Document attach

`attach_document` calls `klarbog_storage(company)` first. Metadata stays in `documents.json`; `path_hint` is relative (e.g. `attachments/scan.pdf`).

## Related

- ADR-007: `docs/adr/ADR-007-cloudflare-eu.md`
- Documents skill: `docs/skills/documents-exceptions.md`
