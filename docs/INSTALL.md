# Install Klarbog (from source)

Klarbog is **DEV software** — loopback HTTP by default, local SQLite, no
production SLA yet. Build from the `rust-dev` branch unless release tags say
otherwise.

### HTTP bind (ADR-014; fail-closed)

`klarbog-api` listens on **`127.0.0.1:3195`** unless you set `KLARBOG_BIND`.
Non-loopback bind is **opt-in only** and never the default:

| Variable | Default | Purpose |
|----------|---------|---------|
| `KLARBOG_BIND` | unset → `127.0.0.1:3195` | Explicit `host:port` |
| `KLARBOG_ALLOW_NON_LOOPBACK` | unset / `0` | Must be `1` **and** paired with explicit `KLARBOG_BIND` for non-loopback |

Without the allow flag, a non-loopback `KLARBOG_BIND` makes the process **exit
at startup**. The allow flag alone does not widen the listen address.

**Residual risks** if you opt in: no TLS inside `klarbog-api` (terminate at a
reverse proxy — [ADR-015](adr/ADR-015-tls-edge.md)), auth is still actor headers
+ allowlist (not production IdP), and backups remain operator-owned. This gate
is a scaffold — **not** a public deploy authorization.

See:
- [`docs/adr/ADR-014-production-posture.md`](adr/ADR-014-production-posture.md)
- [`docs/adr/ADR-015-tls-edge.md`](adr/ADR-015-tls-edge.md)
- [`docs/adr/ADR-016-api-token.md`](adr/ADR-016-api-token.md) — optional
  `KLARBOG_API_TOKEN` bearer gate
- [`docs/adr/ADR-017-session-cookie.md`](adr/ADR-017-session-cookie.md) —
  optional HMAC `klarbog_session` cookie when `KLARBOG_SESSION_SECRET` is set
- Operator checklist: [`docs/skills/production-checklist.md`](skills/production-checklist.md)

### Optional API token (ADR-016)

When `KLARBOG_API_TOKEN` is set (non-empty), `/api/v1/*` requires
`Authorization: Bearer <token>` or `x-klarbog-api-token`. Unset = DEV (no
bearer). Exempt: `/health`, `/ui/*`, `POST /api/v1/webhooks/stripe` (HMAC),
and auth login/logout when session is enabled. Actor headers remain required
on company routes. Configure the same secret in the web UI under
**Indstillinger** when the env is set.

### Optional session cookie (ADR-017)

When **both** `KLARBOG_API_TOKEN` and `KLARBOG_SESSION_SECRET` are set:

- `POST /api/v1/auth/login` with `{ "token": "<api-token>" }` sets HttpOnly
  `klarbog_session` (HMAC-SHA256; `SameSite=Lax`; `Secure` only with
  `KLARBOG_SESSION_COOKIE_SECURE=1` or non-loopback bind posture).
- Middleware accepts Bearer **or** a valid session cookie.
- `POST /api/v1/auth/logout` clears the cookie.
- Unset session secret → no session routes (Bearer-only).

Out of scope: OIDC IdP, CSRF for cross-site, multi-user accounts.

## Requirements

- **Rust** 1.75+ (2021 edition workspace)
- **Linux or macOS** recommended (Windows may work; not CI-tested)
- **curl** (health checks)
- Optional: Revolut Business API token and/or OAuth client, Stripe secret key +
  webhook secret, Cloudflare R2 credentials

Copy [`.env.example`](../.env.example) for documented `KLARBOG_*` names (leave values
empty for offline CSV / local disk). Never commit a filled `.env`.

## Clone and build

```bash
git clone https://github.com/King4s/klarbog.git
cd klarbog
git checkout rust-dev

cargo build --workspace --release
# or: ./scripts/package-dev.sh   # same build + copies to dist/
```

Binaries (release):

| Binary | Crate | Purpose |
|--------|-------|---------|
| `klarbog` | `klarbog-cli` | init, demo, backup, retention, GDPR export |
| `klarbog-api` | `klarbog-api` | HTTP REST on loopback |
| `klarbog-mcp` | `klarbog-mcp` | MCP stdio tools for AI assistants |

Default paths after `cargo build --release`:

```
target/release/klarbog
target/release/klarbog-api
target/release/klarbog-mcp
```

Optional install prefix (example):

```bash
install -m 755 target/release/klarbog{,-api,-mcp} ~/.local/bin/
```

## Verify the tree

```bash
./scripts/verify.sh
```

Expect the last line: `VERIFY_OK` (fmt, clippy, tests, line gates).

Offline HTTP contract smoke (axum oneshot, no bind/network) — includes moms-suggest
and multi-currency import preview **400**:

```bash
./scripts/contract-smoke.sh
# or: cargo test -p klarbog-api contract_smoke
```

### Optional live E2E (secrets; loopback only)

When **Stripe + R2** DEV secrets are exported in the shell, run a bounded live
smoke against loopback `klarbog-api` (health, status, bank import preview
`source=api` for Stripe). **Without Stripe/R2 the script fails closed:** prints
`SKIP`, exits 0, and makes no live provider calls.

`KLARBOG_REVOLUT_API_TOKEN` is optional: without a Revolut Business account the
Revolut preview stays **dormant** (explicit SKIP line; no Revolut call). Export
the token later to enable that leg.

```bash
./scripts/live-e2e.sh
# SKIP path ends with LIVE_E2E_SKIP; success ends with LIVE_E2E_OK
```

Required for the live path (never commit or print):
`KLARBOG_STRIPE_SECRET_KEY`, and
`KLARBOG_R2_ACCOUNT_ID` / `KLARBOG_R2_ACCESS_KEY_ID` /
`KLARBOG_R2_SECRET_ACCESS_KEY` / `KLARBOG_R2_BUCKET`.
Optional: `KLARBOG_REVOLUT_API_TOKEN`.

See [`docs/skills/live-e2e.md`](skills/live-e2e.md) and
[`docs/adr/ADR-013-live-e2e.md`](adr/ADR-013-live-e2e.md). Live E2E stays on
loopback; production bind remains gated (ADR-014), never enabled by default.

## Data layout and isolation

Company data lives under directories you create (typically `companies/<slug>/`).
The API and MCP refuse paths outside an allowlist root.

| Variable | Default | Purpose |
|----------|---------|---------|
| `KLARBOG_ALLOWLIST_ROOT` | repository root at build time | Only company dirs under this tree are writable |
| `ALLOWLIST_ROOT` | alias for above | Used by `./scripts/verify.sh` |

Example:

```bash
export KLARBOG_ALLOWLIST_ROOT="$HOME/klarbog-data"
mkdir -p "$KLARBOG_ALLOWLIST_ROOT/companies"
```

Initialize a company:

```bash
klarbog init \
  --company "$KLARBOG_ALLOWLIST_ROOT/companies/min-aps" \
  --name "Min ApS" \
  --actor owner
```

## Run the HTTP API

Binds **127.0.0.1:3195** only (not exposed to the network by default).

```bash
export KLARBOG_ALLOWLIST_ROOT="${KLARBOG_ALLOWLIST_ROOT:-$(pwd)}"
./target/release/klarbog-api
```

Health check:

```bash
curl -sS http://127.0.0.1:3195/health
# {"ok":true,"service":"klarbog-api",...}
```

### Local web UI (ADR-012)

Same process serves Klarbog-owned static assets from repo `ui/` (no separate UI
process). After `cargo run -p klarbog-api` (or a release binary), open:

```text
http://127.0.0.1:3195/ui/
```

Set company path + actor under **Indstillinger** (stored in `localStorage`).

| Variable | Default | Purpose |
|----------|---------|---------|
| `KLARBOG_UI_DIR` | repo `ui/` (next to the workspace) | Override static asset root if the binary is moved away from the tree |

If the directory is missing, `/ui` returns 404 until `ui/` exists or
`KLARBOG_UI_DIR` points at a valid tree.

Optional client env:

| Variable | Default |
|----------|---------|
| `KLARBOG_API_BASE` | `http://127.0.0.1:3195` |
| `KLARBOG_COMPANY` | absolute path to active company dir |

REST surface (v1): journal preview/commit + optional `moms-suggest` (`#vat25`
net+vat i64 legs, never posts); CRM parties; invoice drafts / status /
mark-paid / mark-part-paid (payment ledger remaining; optional `preview` →
confirm-token); bank import preview;
reconcile suggest/apply (`preview` → confirm-token); Stripe webhook + consume +
reconcile-suggest; reconcile-apply-preview; Revolut OAuth start/callback/refresh; documents (+ delete);
exceptions; retention / purge / backup; GDPR export + party erase. See
[`docs/skills/`](skills/) and [`docs/agent-setup/prompt.md`](agent-setup/prompt.md).

## Run MCP (AI tools)

Stdio JSON-RPC server — one request per line (MCP-shaped `tools/list`, `tools/call`).

```bash
export KLARBOG_ALLOWLIST_ROOT="${KLARBOG_ALLOWLIST_ROOT:-$(pwd)}"
./target/release/klarbog-mcp
```

Wire it in your AI client (Cursor, Claude Desktop, etc.) as a **stdio MCP server**
pointing at the `klarbog-mcp` binary. Mutating tools use **two-phase confirm**
(preview token → commit). Useful tools include `klarbog_status` (HTTP
`/api/v1/status` parity: mode, allowlist, plugins; `bind=stdio`),
`journal_moms_post_suggestion`
(preview-only VAT split hint), `rules_chart_list` (HTTP `GET /api/v1/rules/chart`
parity — DEV stub codes + labels; read-only; no journal write),
`crm_upsert_party` / `crm_list_parties` (HTTP CRM
parity; no journal write), `invoice_create_draft` / `invoice_list` /
`invoice_patch_status` plus mark-paid previews (HTTP invoice lifecycle parity;
optional `preview:true` → ConfirmStore token; suggestions only — no journal write), `documents_attach` / `documents_list` /
`documents_delete` and `exceptions_raise` / `exceptions_list` / `exceptions_set_open`
(HTTP documents + exceptions parity; no journal write), `gdpr_export`
(company-scoped metadata → `gdpr_export.json`; no binary blobs), `gdpr_erase_party`,
`retention_purge` (dry-run/`confirm`; closed exceptions + optional orphan-doc GC;
journal untouched), `bank_reconcile_suggest` / `bank_reconcile_apply` (`force` only
for `user` below safe threshold; optional `preview:true` → ConfirmStore token),
Revolut OAuth MCP `revolut_oauth_start` / `revolut_oauth_callback` /
`revolut_oauth_refresh` (never echo tokens; secrets mode `0600`),
and Stripe pipelines
`bank_stripe_reconcile_suggest` / `bank_stripe_reconcile_apply_preview`
(consume → suggest / unique-safe ConfirmStore preview; no auto journal post —
see [`docs/skills/bank-import.md`](skills/bank-import.md)).

**End-user AI instructions:** give your assistant
[`docs/agent-setup/prompt.md`](agent-setup/prompt.md) — one markdown file that
teaches any model how to talk to your installation (MCP, HTTP, CLI).

Developers editing the repo: [`docs/agent-setup/DEVELOPER.md`](agent-setup/DEVELOPER.md).

## Run the CLI

```bash
export KLARBOG_ALLOWLIST_ROOT="${KLARBOG_ALLOWLIST_ROOT:-$(pwd)}"

# End-to-end smoke (temp company, fixtures; includes moms-suggest i64 #vat25)
cargo run -p klarbog-cli -- demo
# or: ./target/release/klarbog demo

klarbog retention --company "$KLARBOG_ALLOWLIST_ROOT/companies/min-aps"
klarbog backup   --company "$KLARBOG_ALLOWLIST_ROOT/companies/min-aps"
# → company backups/<ts>/manifest.json + manifest.sha256 (operator-owned;
#   not a managed HA/DR SLA). Restore drills: scratch allowlist only.
#   Skill: docs/skills/backup-restore.md — offline: ./scripts/backup-smoke.sh
klarbog gdpr-export --company "$KLARBOG_ALLOWLIST_ROOT/companies/min-aps"
# dry-run party erasure (add --confirm to apply; optional --delete-documents)
klarbog gdpr-erase-party \
  --company "$KLARBOG_ALLOWLIST_ROOT/companies/min-aps" \
  --party-id party_example
```

## Payment rails (optional)

Bank import supports **live API** for Revolut and Stripe; CSV remains an offline
fallback. Tokens belong in the **API process environment** (or shell that starts
`klarbog-api` / `klarbog-mcp`), never in git.

### Revolut Business API

| Variable | Required | Notes |
|----------|----------|-------|
| `KLARBOG_REVOLUT_API_TOKEN` | yes (for `source=api` without OAuth secrets) | Secret — never commit or print |
| `KLARBOG_REVOLUT_API_BASE` | no | Override API base URL |
| `KLARBOG_REVOLUT_CLIENT_ID` | OAuth flow | Public client id |
| `KLARBOG_REVOLUT_CLIENT_SECRET` | OAuth flow | Secret — never commit or print |
| `KLARBOG_REVOLUT_REDIRECT_URI` | OAuth flow | Must match registered redirect |

Import preview: `provider=revolut`, `source=api` (or `source=csv` with pasted export).
OAuth: `/api/v1/revolut/oauth/start|callback|refresh` and MCP
`revolut_oauth_start` / `revolut_oauth_callback` / `revolut_oauth_refresh`
(tokens under company `secrets/`, mode `0600`; responses never include raw tokens).

See [`docs/adr/ADR-008-revolut-bank.md`](adr/ADR-008-revolut-bank.md).

### Stripe

| Variable | Required | Notes |
|----------|----------|-------|
| `KLARBOG_STRIPE_SECRET_KEY` | yes (for `source=api`) | Secret — never commit or print |
| `KLARBOG_STRIPE_API_BASE` | no | Default `https://api.stripe.com` |
| `KLARBOG_STRIPE_WEBHOOK_SECRET` | webhooks | HMAC verify on `/api/v1/webhooks/stripe` |

Import preview: `provider=stripe`, `source=api` (or CSV balance export offline).
Consume queued webhook drafts: `POST /api/v1/bank/stripe/consume` (dry-run default;
`confirm:true` to mark consumed — still no journal post).

See [`docs/adr/ADR-009-stripe.md`](adr/ADR-009-stripe.md).

### Generic Danish bank CSV

No extra env. Use `provider=generic_dk`, `source=csv` with semicolon-separated export.

## Object storage — Cloudflare R2 (EU)

Attachments default to **local disk** under each company (`objects/`). For R2:

| Variable | Required when `KLARBOG_STORAGE=r2` |
|----------|-------------------------------------|
| `KLARBOG_STORAGE` | set to `r2` (default `local`) |
| `KLARBOG_R2_ACCOUNT_ID` | yes |
| `KLARBOG_R2_ACCESS_KEY_ID` | yes |
| `KLARBOG_R2_SECRET_ACCESS_KEY` | yes |
| `KLARBOG_R2_BUCKET` | yes |
| `KLARBOG_R2_JURISDICTION` | no (default EU / WEUR) |
| `KLARBOG_R2_ALLOW_NON_EU` | no — set `1` only to override EU fail-closed |

See [`docs/skills/storage-r2.md`](skills/storage-r2.md) and
[`docs/adr/ADR-007-cloudflare-eu.md`](adr/ADR-007-cloudflare-eu.md).

## Package helper (DEV)

```bash
./scripts/package-dev.sh
```

Builds `--release` and copies the three binaries into `dist/`. The `dist/` directory
is gitignored; use it for local bundles or CI artifacts.

By default the script prints **sha256** digests of `dist/klarbog`,
`dist/klarbog-api`, and `dist/klarbog-mcp` (set `PACKAGE_DEV_NO_SHA256=1` to skip).
You can re-check later with:

```bash
sha256sum dist/klarbog dist/klarbog-api dist/klarbog-mcp
# macOS: shasum -a 256 dist/klarbog dist/klarbog-api dist/klarbog-mcp
```

After packaging (or any release build), run the offline contract smoke:

```bash
./scripts/contract-smoke.sh
```

Expect `CONTRACT_SMOKE_OK`. Full tree gate remains `./scripts/verify.sh` → `VERIFY_OK`.

## Credits and license

Klarbog is a Rust port of
[Rentemester](https://github.com/mikkelkrogsholm/rentemester) (MIT). See
[`NOTICE`](../NOTICE) and [`LICENSE`](../LICENSE).

Upstream TypeScript reference: [`reference/typescript/`](../reference/typescript/).

## Next steps

- Product changelog: [`CHANGELOG.md`](../CHANGELOG.md)
- Architecture decisions: [`docs/adr/`](adr/)
- Agent skills: [`docs/skills/`](skills/)
- Roadmap: [`swarm/ROADMAP.md`](../swarm/ROADMAP.md)
