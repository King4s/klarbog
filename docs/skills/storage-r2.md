---
name: klarbog-storage-r2
description: >-
  Object storage for Klarbog attachments (local DEV default, Cloudflare R2 EU
  SigV4 put/get). Use when attaching documents or wiring binary upload paths.
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
| `r2` | Validate `KLARBOG_R2_*` via [`R2Config::from_env`]; use [`R2Store`] SigV4 put/get |

```rust
use klarbog_storage::{klarbog_storage, KlarbogStorage};

let backend = klarbog_storage(company_path)?;
match backend {
    KlarbogStorage::Local(store) => { /* path_hint under objects/ */ }
    KlarbogStorage::R2(cfg) => {
        let store = klarbog_storage::R2Store::new(cfg);
        store.put("attachments/scan.pdf", &bytes).await?;
    }
}
```

## R2 env (never commit secrets)

- `KLARBOG_R2_ACCOUNT_ID`
- `KLARBOG_R2_ACCESS_KEY_ID`
- `KLARBOG_R2_SECRET_ACCESS_KEY`
- `KLARBOG_R2_BUCKET`
- Optional: `KLARBOG_R2_JURISDICTION` (default `eu` / WEUR)
- Override non-EU only with `KLARBOG_R2_ALLOW_NON_EU=1`

## SigV4 I/O (live)

[`R2Store::put`] and [`R2Store::get`] call the R2 S3-compatible API with **AWS SigV4** (`region=auto`, path-style `/{bucket}/{key}`). Implementation is in `klarbog-storage` (`sigv4.rs` + `reqwest`); no extra AWS SDK.

- **Fail-closed non-EU**: rejected at config load and again before each request unless `KLARBOG_R2_ALLOW_NON_EU=1`.
- **Delete** remains `NotImplementedInDev` (optional follow-up).
- **Tests** exercise signing and key validation only — no network in `cargo test`.

Stage smoke (owner creds, not CI): `KLARBOG_STORAGE=r2` + env vars, then put/get a small object under `attachments/…`.

## Document attach

`attach_document` calls `klarbog_storage(company)` first. Metadata stays in `documents.json`; `path_hint` is relative (e.g. `attachments/scan.pdf`).

## Related

- ADR-007: `docs/adr/ADR-007-cloudflare-eu.md`
- Documents skill: `docs/skills/documents-exceptions.md`
