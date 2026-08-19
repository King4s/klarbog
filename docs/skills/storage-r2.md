---
name: klarbog-storage-r2
description: >-
  Object storage for Klarbog attachments (local DEV default, Cloudflare R2 EU
  SigV4 put/get/delete). Use when attaching documents or wiring binary upload paths.
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
| `r2` | Validate `KLARBOG_R2_*` via [`R2Config::from_env`]; use [`R2Store`] SigV4 put/get/delete |

```rust
use klarbog_storage::{klarbog_storage, KlarbogStorage};

let backend = klarbog_storage(company_path)?;
backend.put("attachments/scan.pdf", &bytes).await?;
backend.delete("attachments/scan.pdf").await?;
match backend {
    KlarbogStorage::Local(_) => { /* under <company>/objects/ */ }
    KlarbogStorage::R2(_) => { /* same key in R2 bucket */ }
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

[`R2Store::put`], [`R2Store::get`], and [`R2Store::delete`] call the R2 S3-compatible API with **AWS SigV4** (`region=auto`, path-style `/{bucket}/{key}`). Implementation is in `klarbog-storage` (`sigv4.rs` + `reqwest`); no extra AWS SDK.

- **Fail-closed non-EU**: rejected at config load and again before each request unless `KLARBOG_R2_ALLOW_NON_EU=1`.
- **DeleteObject**: signed DELETE with empty payload (idempotent on R2; local fs returns `NotFound` if missing).
- **Tests** exercise signing and key validation only — no network in `cargo test`.

Stage smoke (owner creds, not CI): `KLARBOG_STORAGE=r2` + env vars, then put/get a small object under `attachments/…`.

## Document attach / remove

`attach_document` selects storage via `klarbog_storage(company)`, optionally `put`s bytes at `path_hint`, then writes `documents.json` metadata. HTTP clients may send `content_base64` instead of in-process `content`.

`remove_document` (HTTP `DELETE /api/v1/documents`) drops metadata and, by default (`delete_object: true`), deletes the object at `path_hint` via [`KlarbogStorage::delete`]. Missing objects are ignored so metadata cleanup still succeeds.

## Related

- ADR-007: `docs/adr/ADR-007-cloudflare-eu.md`
- Documents skill: `docs/skills/documents-exceptions.md`
