# Install Klarbog (from source)

Klarbog is **DEV software** — loopback HTTP, local SQLite, no production SLA yet.
Build from the `rust-dev` branch unless release tags say otherwise.

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
git clone https://github.com/mikkelkrogsholm/klarbog.git
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

Optional client env:

| Variable | Default |
|----------|---------|
| `KLARBOG_API_BASE` | `http://127.0.0.1:3195` |
| `KLARBOG_COMPANY` | absolute path to active company dir |

REST surface (v1): journal preview/commit + optional `moms-suggest` (`#vat25`
net+vat i64 legs, never posts); CRM parties; invoice drafts / status /
mark-paid / mark-part-paid (payment ledger remaining); bank import preview;
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
(preview token → commit). Useful tools include `journal_moms_post_suggestion`
(preview-only VAT split hint), `gdpr_export` (company-scoped metadata →
`gdpr_export.json`; no binary blobs), `gdpr_erase_party`, `retention_purge`
(dry-run/`confirm`; closed exceptions + optional orphan-doc GC; journal untouched),
`bank_reconcile_suggest` / `bank_reconcile_apply` (`force` only for `user` below safe
threshold), and Stripe
pipelines `bank_stripe_reconcile_suggest` / `bank_stripe_reconcile_apply_preview`
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
OAuth: `/api/v1/revolut/oauth/start|callback|refresh` (tokens under company `secrets/`, mode `0600`).

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
