# Klarbog

**Agent-first bogføring til Danmark** — Rust-ledger med MCP, HTTP API og CLI.

| | |
|---|---|
| **Repo** | [github.com/King4s/klarbog](https://github.com/King4s/klarbog) |
| **Branch** | `rust-dev` (aktiv udvikling) |
| **Status** | **DEV** — loopback API, lokal data, ingen produktion-SLA endnu |
| **Licens** | MIT ([`LICENSE`](LICENSE)) |

## Hvad det er

Klarbog er et **lokalt / privat** bogføringssystem til dansk regnskab, bygget så en AI (eller dig) kan styre det via tools — ikke via et tungt UI først.

**Ufravigelige regler:**

1. Penge er **`i64` minor units** (øre) — aldrig `f64` til beløb  
2. Journalen er **dobbelt bogholderi** (debet = kredit)  
3. Journal-skrivning er **to-fase**: preview → confirm-token → commit  
4. Firmadata ligger under en **allowlist** (`KLARBOG_ALLOWLIST_ROOT`)  
5. Muterende kald kræver en **actor** i virksomhedens `policy.json`

## Overflader

| Overflade | Binær | Formål |
|-----------|--------|--------|
| **MCP** | `klarbog-mcp` | AI-tools (journal, bank, faktura, CRM, dokumenter, retention, …) |
| **HTTP** | `klarbog-api` | REST på `127.0.0.1:3195` |
| **UI** | (serveres af API) | Menneske-GUI på `http://127.0.0.1:3195/ui/` (ADR-012) |
| **CLI** | `klarbog` | init, demo, backup, retention, GDPR-export |

### Lokal web-UI (DEV)

```bash
cargo run -p klarbog-api
```

Åbn [http://127.0.0.1:3195/ui/](http://127.0.0.1:3195/ui/). Statiske filer i `ui/` serveres same-origin af API’en (valgfri `KLARBOG_UI_DIR`). Se [`docs/INSTALL.md`](docs/INSTALL.md) og [`docs/adr/ADR-012-local-web-ui.md`](docs/adr/ADR-012-local-web-ui.md).

Giv enhver AI **én prompt-fil**, så den selv lærer at snakke med din installation:

📄 **[`docs/agent-setup/prompt.md`](docs/agent-setup/prompt.md)**

## Kanter i produktet (DEV)

| Område | Indhold |
|--------|---------|
| **Bank** | Revolut Business API + OAuth, Stripe API + webhooks, GenericDk CSV; reconcile suggest/apply; multi-valuta fail-closed vs firmavaluta |
| **Faktura / CRM** | Parter, kladder, status, mark-paid / part-paid (journal-forslag — ingen auto-post) |
| **Moms (DK)** | `#vat25` forslag (`moms-suggest`) — preview only, `auto_post: false` |
| **Bilag** | Dokumenter + exceptions; lokal disk eller valgfri **Cloudflare R2** (EU fail-closed) |
| **Retention / GDPR** | Backup-manifest, company-scoped export, party erase (journal forbliver urørlig), purge dry-run/confirm |

## Hurtig start

```bash
git clone https://github.com/King4s/klarbog.git
cd klarbog
git checkout rust-dev

cargo build --workspace
./scripts/verify.sh          # forvent: VERIFY_OK
./scripts/contract-smoke.sh  # offline HTTP-kontrakt (valgfrit)
cargo run -p klarbog-cli -- demo
# UI (samme process som API):
cargo run -p klarbog-api
# åbn http://127.0.0.1:3195/ui/
```

Env-skabelon (ingen secrets): [`.env.example`](.env.example)  

**Installér og kør (API / MCP / CLI, env, R2, packaging):** [`docs/INSTALL.md`](docs/INSTALL.md)

Valgfri release-bundle:

```bash
./scripts/package-dev.sh   # binærer → dist/ (+ valgfri sha256)
```

## Dokumenter

| | |
|---|---|
| Skills (agent-workflows) | [`docs/skills/`](docs/skills/) |
| ADR’er | [`docs/adr/`](docs/adr/) |
| Udvikler-bootstrap | [`docs/agent-setup/DEVELOPER.md`](docs/agent-setup/DEVELOPER.md) |
| Roadmap / swarm | [`swarm/ROADMAP.md`](swarm/ROADMAP.md) |
| Changelog | [`CHANGELOG.md`](CHANGELOG.md) |

## Oprindelse og credits

Klarbog er en **Rust-port** af open source-projektet
**[Rentemester](https://github.com/mikkelkrogsholm/rentemester)** (TypeScript, MIT).

Tak til **Mikkel Krogsholm** og Rentemester-bidragsydere for domænemodel og
agent-venligt design. Klarbogs Rust-workspace er ny kode; upstream-regler og
fixtures under [`reference/typescript/`](reference/typescript/) er bevaret til
sammenligning.

- Upstream: https://github.com/mikkelkrogsholm/rentemester  
- Attribution: [`NOTICE`](NOTICE)  
- Dette repo: https://github.com/King4s/klarbog  
