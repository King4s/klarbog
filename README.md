# Klarbog

**Agent-first bogføring til Danmark** — Rust-ledger med MCP, HTTP API og CLI.  
Forket fra [Rentemester](https://github.com/mikkelkrogsholm/rentemester) (MIT — se `LICENSE` + `NOTICE`).

Status: **DEV** (loopback API, lokal data). Ikke produktion endnu.

## Produktpunkter

### 1. AI-prompt med i pakken

Når Klarbog er installeret, kan brugeren give **én markdown-fil** til *hvilken som helst* AI. AI’en lærer selv at snakke med produktet (MCP / HTTP / CLI).

📄 **[`docs/agent-setup/prompt.md`](docs/agent-setup/prompt.md)**

### 2. Betalingsrails med fuld API

| Rail | Integration |
|------|-------------|
| **Revolut** | Business **API** (CSV kun offline-fallback) |
| **Stripe** | **API** (balance/payouts; CSV kun offline-fallback) |
| **GenericDk** | Bank-CSV (semikolon) |

### 3. Cloudflare R2 (EU)

Object storage til bilag — lokal disk i DEV; valgfrit **R2** med EU fail-closed (`KLARBOG_STORAGE=r2`).

---

## Overflader

| Overflade | Formål |
|-----------|--------|
| **MCP** (`klarbog-mcp`) | AI-tools: journal, bank-import, backup, … |
| **HTTP** (`klarbog-api`) | REST på `127.0.0.1:3195` |
| **CLI** (`klarbog`) | init, demo, backup, retention, GDPR-export |

Penge er `i64` minor units (øre) — aldrig `f64`. Dobbelt bogholderi er obligatorisk. Journal-skrivning er to-fase (preview → commit).

## Hurtig start (udvikler)

```bash
cargo build --workspace
./scripts/verify.sh
cargo run -p klarbog-cli -- demo
```

## Mere

- Skills: [`docs/skills/`](docs/skills/)
- ADR: [`docs/adr/`](docs/adr/)
- Roadmap: [`swarm/ROADMAP.md`](swarm/ROADMAP.md)
- Udvikler-bootstrap: [`docs/agent-setup/DEVELOPER.md`](docs/agent-setup/DEVELOPER.md)
- Upstream TS-reference: `reference/typescript/` (ikke produktdok)
