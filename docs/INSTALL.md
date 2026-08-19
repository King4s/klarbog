# Install Klarbog (from source)

Klarbog is **DEV software** — loopback HTTP, local SQLite, no production SLA yet.
Build from the `rust-dev` branch unless release tags say otherwise.

## Requirements

- **Rust** 1.75+ (2021 edition workspace)
- **Linux or macOS** recommended (Windows may work; not CI-tested)
- **curl** (health checks)
- Optional: Revolut Business API token, Stripe secret key, Cloudflare R2 credentials

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

REST surface (v1): journal preview/commit, bank import preview, CRM parties,
invoice drafts, documents, retention, backup, GDPR export. See
[`docs/skills/`](skills/) for tool semantics.

## Run MCP (AI tools)

Stdio JSON-RPC server — one request per line (MCP-shaped `tools/list`, `tools/call`).

```bash
export KLARBOG_ALLOWLIST_ROOT="${KLARBOG_ALLOWLIST_ROOT:-$(pwd)}"
./target/release/klarbog-mcp
```

Wire it in your AI client (Cursor, Claude Desktop, etc.) as a **stdio MCP server**
pointing at the `klarbog-mcp` binary. Mutating tools use **two-phase confirm**
(preview token → commit).

**End-user AI instructions:** give your assistant
[`docs/agent-setup/prompt.md`](agent-setup/prompt.md) — one markdown file that
teaches any model how to talk to your installation (MCP, HTTP, CLI).

Developers editing the repo: [`docs/agent-setup/DEVELOPER.md`](agent-setup/DEVELOPER.md).

## Run the CLI

```bash
export KLARBOG_ALLOWLIST_ROOT="${KLARBOG_ALLOWLIST_ROOT:-$(pwd)}"

# End-to-end smoke (temp company, fixtures)
cargo run -p klarbog-cli -- demo
# or: ./target/release/klarbog demo

klarbog retention --company "$KLARBOG_ALLOWLIST_ROOT/companies/min-aps"
klarbog backup   --company "$KLARBOG_ALLOWLIST_ROOT/companies/min-aps"
klarbog gdpr-export --company "$KLARBOG_ALLOWLIST_ROOT/companies/min-aps"
```

## Payment rails (optional)

Bank import supports **live API** for Revolut and Stripe; CSV remains an offline
fallback. Tokens belong in the **API process environment** (or shell that starts
`klarbog-api` / `klarbog-mcp`), never in git.

### Revolut Business API

| Variable | Required | Notes |
|----------|----------|-------|
| `KLARBOG_REVOLUT_API_TOKEN` | yes (for `source=api`) | Secret — never commit or print |
| `KLARBOG_REVOLUT_API_BASE` | no | Override API base URL |

Import preview: `provider=revolut`, `source=api` (or `source=csv` with pasted export).

See [`docs/adr/ADR-008-revolut-bank.md`](adr/ADR-008-revolut-bank.md).

### Stripe

| Variable | Required | Notes |
|----------|----------|-------|
| `KLARBOG_STRIPE_SECRET_KEY` | yes (for `source=api`) | Secret — never commit or print |
| `KLARBOG_STRIPE_API_BASE` | no | Default `https://api.stripe.com` |

Import preview: `provider=stripe`, `source=api` (or CSV balance export offline).

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
