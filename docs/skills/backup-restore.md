---
name: klarbog-backup-restore
description: >-
  Operator backup via CLI (manifest + SHA-256 sidecar). What is included,
  restore expectations on a scratch allowlist, and that this is not a managed
  HA / DR SLA.
---

# Backup and restore (operator-owned)

Klarbog backup helpers are **DEV aids** for integrity checklists. They are
**not** a managed high-availability or disaster-recovery SLA. Operators own
off-host copies, retention schedules, and restore drills.

## When to use

- After meaningful company data changes, before risky upgrades, or as part of
  the [production checklist](production-checklist.md).
- To produce a dated manifest of tracked company files + journal digests.
- **Not** as a substitute for filesystem / volume / object-store backups of the
  full company directory (including `objects/` blobs if present).

## CLI

```bash
export KLARBOG_ALLOWLIST_ROOT="${KLARBOG_ALLOWLIST_ROOT:-$(pwd)}"

klarbog backup --company "$KLARBOG_ALLOWLIST_ROOT/companies/min-aps"
```

Writes under the company dir:

| Artifact | Path pattern |
|----------|----------------|
| Manifest | `backups/<unix_ms>/manifest.json` |
| Sidecar | `backups/<unix_ms>/manifest.sha256` |

Stdout is a JSON envelope whose `data` includes `backup_key`, `files[]`
(path / sha256 / size), `journal_digests`, plus party / invoice / document
refs. Never commit backup trees that contain production personal data into git.

HTTP / MCP equivalents (same writer): `POST /api/v1/backup`, MCP
`backup_manifest` (AuthZ + allowlist).

## What the manifest includes

Tracked **files** (when present on disk):

- `policy.json`, `retention.json`
- `parties.json`, `invoices.json`, `documents.json`, `exceptions.json`
- expense memo template path, `gdpr_export.json`
- `ledger.sqlite`

Also embedded in the manifest JSON (not necessarily separate files):

- Journal digest summaries from the SQLite store
- Party / invoice / document id refs for inventory

**Not** covered as a full DR package by this helper alone:

- Arbitrary files under the company tree outside the tracked list
- Object blobs under `objects/` (local) or remote R2 objects
- Secrets under `secrets/` (mode `0600`) — back those up with operator policy
- Multi-node replication, PITR, or automated failover

## Restore expectations (scratch allowlist)

There is **no** first-class `klarbog restore` command. Restore is operator-led:

1. Provision a **scratch** allowlist root (empty temp tree or dedicated path).
   Set `KLARBOG_ALLOWLIST_ROOT` to that root only — never overwrite a live
   company in place on first try.
2. Copy the company directory (or at least every path listed in
   `manifest.json` `files[]`, plus any blobs you need) into a path **under**
   that allowlist.
3. Confirm `manifest.sha256` still matches the on-disk `manifest.json` bytes
   (sidecar must be 64 hex chars of the file SHA-256).
4. Optionally re-run `klarbog backup` on the restored company and compare
   digests / journal summaries against the original manifest.
5. Point CLI / API / MCP at the scratch root and smoke-read status, parties,
   and a journal digest before promoting.

Fail closed: if the sidecar does not match, treat the manifest as untrusted.

## Offline smoke

```bash
./scripts/backup-smoke.sh
```

Inits a temp company under a scratch allowlist, runs `klarbog backup`, asserts
`manifest.json` (+ sidecar) exist, prints `BACKUP_SMOKE_OK`. No network. Full
tree gate remains `./scripts/verify.sh` → `VERIFY_OK`.

## Hard rules

- Operator-owned plan; not a managed HA / multi-node SLA.
- Scratch allowlist for restore drills; do not practice restore onto live data.
- No secrets in chat, git, or smoke logs.
- No sister-product names in operator runbooks.

## See also

- [production-checklist.md](production-checklist.md) § Data isolation and backup
- [ADR-014 production posture](../adr/ADR-014-production-posture.md) (residual: backups)
- [INSTALL](../INSTALL.md) — CLI backup examples
