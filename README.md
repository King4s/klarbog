# Klarbog

**Agent-first bogføring til Danmark** — Rust-ledger med MCP, HTTP API og CLI.  
Forket fra [Rentemester](https://github.com/mikkelkrogsholm/rentemester) (MIT — se `LICENSE` + `NOTICE`).

Status: **DEV** (loopback API, lokal data). Ikke produktion endnu.

## Produktpunkt: AI-prompt med i pakken

Når Klarbog er installeret, kan brugeren give **én markdown-fil** til *hvilken som helst* AI (ChatGPT, Claude, Cursor, Codex, Gemini, …). AI’en lærer så selv, hvordan den skal snakke med produktet — MCP-tools, HTTP-ruter, CLI, to-fase journal, øre som heltal, Revolut/DK-bank, osv.

📄 **[`docs/agent-setup/prompt.md`](docs/agent-setup/prompt.md)** ← den fil

Mønster som Cloudflare’s [agent-setup prompt](https://developers.cloudflare.com/agent-setup/prompt.md), men rettet mod **at bruge Klarbog**, ikke at sætte Cloudflare-udviklermiljø op.

*(Udviklere der hacker på kildekoden: [`docs/agent-setup/DEVELOPER.md`](docs/agent-setup/DEVELOPER.md).)*

## Overflader

| Overflade | Formål |
|-----------|--------|
| **MCP** (`klarbog-mcp`) | AI kalder tools (journal preview/commit, bank-import, backup, …) |
| **HTTP** (`klarbog-api`) | REST på `127.0.0.1:3195` |
| **CLI** (`klarbog`) | init, demo, backup, retention, GDPR-export |

Penge er `i64` minor units (øre) — aldrig `f64`. Dobbelt bogholderi er obligatorisk. Journal-skrivning er to-fase (preview → commit).

## Hurtig start (udvikler)

```bash
cargo build --workspace
cargo test --workspace
./scripts/verify.sh
cargo run -p klarbog-cli -- demo
```

## Mere

- Skills til agenter: [`docs/skills/`](docs/skills/)
- Beslutninger: [`docs/adr/`](docs/adr/)
- Slice-roadmap: [`swarm/ROADMAP.md`](swarm/ROADMAP.md)
- Upstream TypeScript-reference: `reference/typescript/`
