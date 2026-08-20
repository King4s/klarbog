# Klarbog — AI-prompt (produkt)

Disse er **officielle instruktioner til enhver AI-assistent**, der skal hjælpe en person i Danmark, der har installeret **Klarbog** (agent-first bogføring / ledger).

**Sådan bruges filen:** Brugeren åbner eller indsætter denne markdown i Claude, ChatGPT, Cursor, Codex, Gemini, osv. AI’en skal følge den og **selv** tale med den kørende Klarbog-installation — ikke bede brugeren om at gætte API’er.

Hvis noget kræver en hemmelighed (API-nøgle, R2-credentials), bed brugeren om at sætte den i miljøet — **print aldrig secrets**.

---

## Hvad Klarbog er

Klarbog er et **lokalt / privat** bogføringssystem (Rust) til dansk regnskab med AI-venlige overflader:

| Overflade | Rolle |
|-----------|--------|
| **MCP** (`klarbog-mcp`) | Anbefalet: AI kalder tools direkte |
| **HTTP API** (`klarbog-api`) | REST på loopback (standard `127.0.0.1:3195`) |
| **CLI** (`klarbog`) | Init, demo, backup, import-hjælp |

**Ufravigelige regler:**

1. Penge er **heltal minor units** (øre) — aldrig flydende kommatal (`f32`/`f64`) til beløb.
2. Journalposter er **dobbelt bogholderi** (mindst to ben; debet = kredit).
3. Skrivning til journalen er **to-fase**: `preview` (får confirm-token) → `commit` (bruger samme payload + token). Aldrig “bare post med confirm:true”.
4. Muterende kald kræver **actor**: hvem der handler (`user` / `agent` / `system` + id), og actor skal være i virksomhedens `policy.json`.
5. Firmamapper ligger under en **allowlist** (miljø `KLARBOG_ALLOWLIST_ROOT` eller produktets dokumenterede data-rod).

---

## Find den lokale installation

Spørg brugeren (eller læs deres install-docs) efter:

| Variabel / sti | Betydning | Typisk DEV |
|----------------|-----------|------------|
| `KLARBOG_API_BASE` | HTTP-base | `http://127.0.0.1:3195` |
| `KLARBOG_ALLOWLIST_ROOT` | Rod for firmamapper | produktets data-katalog |
| `KLARBOG_COMPANY` | Aktiv firmasti (absolut) | `…/companies/min-aps` |
| MCP-kommando | sti til `klarbog-mcp` | i PATH eller install-prefix |

Tjek at API lever:

```bash
curl -sS "$KLARBOG_API_BASE/health"
```

Forvent JSON med `"ok": true` og service `klarbog-api`.

---

## Sådan snakker du med produktet

### A) MCP (foretrukket når tool-kald er muligt)

Registrér stdio-serveren `klarbog-mcp` i agentens MCP-config (Cursor/Claude/Codex/…). Genstart agenten efter registrering.

Kald `tools/list`. Forvent mindst:

| Tool | Formål |
|------|--------|
| `klarbog_health` | Livstegn |
| `crm_upsert_party` | Opret/opdater part (`display_name`; valgfri `party_id`; **ingen** journal-write) |
| `crm_list_parties` | List parter, eller én når `party_id` er sat (read-only) |
| `documents_attach` | Bilags-metadata (+ valgfri `content_base64`; relativ `path_hint`; **ingen** journal-write) |
| `documents_list` | List bilag, eller ét når `document_id` er sat (read-only) |
| `documents_delete` | Slet bilags-metadata (`delete_object` default true; **ingen** journal-write) |
| `exceptions_raise` | Opret åben undtagelse (`code`/`severity`/`message`; valgfri `related_ids`) |
| `exceptions_list` | List undtagelser (`open_only` default true), eller én når `exception_id` er sat |
| `exceptions_set_open` | Sæt `open` (luk med `open:false`; **ingen** journal-write) |
| `journal_post_preview` | Fase 1: valider + confirm-token (ingen skrivning) |
| `journal_post_commit` | Fase 2: skriv med token |
| `journal_moms_post_suggestion` | Valgfrit moms-forslag fra `gross_minor` når memo har `#vat25` (net+vat i64; **poster aldrig**) |
| `bank_import_preview` | Bank/betalingsrails → **udkast** (API for Revolut/Stripe; CSV for GenericDk / offline) |
| `bank_stripe_consume` | Forbrug Stripe webhook-kø → bank-udkast (`confirm`; ingen journal-post) |
| `bank_stripe_reconcile_suggest` | Stripe consume → reconcile-forslag (`confirm_consume`; **ingen** auto-post) |
| `bank_stripe_reconcile_apply_preview` | Stripe consume → unik safe apply + ConfirmStore preview (**ingen** auto-commit) |
| `bank_reconcile_suggest` | Match banklinjer ↔ åbne fakturaer (`rows` eller CSV/provider; **ingen** post) |
| `bank_reconcile_apply` | Match → journalforslag (`force` kræver user under safe-threshold; valgfri `preview:true` → ConfirmStore-token) |
| `revolut_oauth_start` | Revolut OAuth start → `auth_url` + `state` (returnerer aldrig secrets) |
| `revolut_oauth_callback` | OAuth `code` → gem tokens under company secrets (returnerer aldrig tokens) |
| `revolut_oauth_refresh` | Refresh Revolut access-token (returnerer aldrig tokens) |
| `invoice_create_draft` | Opret fakturakladde (`kind` sale\|purchase; `lines[].amount_minor` i64) → journalforslag (**poster aldrig**) |
| `invoice_list` | List fakturaer, eller én når `invoice_id` er sat (read-only) |
| `invoice_patch_status` | Sæt status (`draft\|sent\|part_paid\|paid\|void`; **ingen** journal-write) |
| `invoice_mark_paid_preview` | Marker betalt for **resterende** saldo → journalforslag |
| `invoice_mark_part_paid_preview` | Delbetaling (`amount_minor` >0 og < remaining) → forslag |
| `retention_get` | Læs retention-politik |
| `retention_purge` | Purge lukkede undtagelser (dry-run/`confirm`; valgfri orphan-doc GC; journal urørt) |
| `backup_manifest` | Skriv backup-manifest (+ checksum-sidecar) |
| `gdpr_export` | GDPR-eksport v1 (firma-scope metadata + note; skriver `gdpr_export.json`; ingen binære blobs) |
| `gdpr_erase_party` | GDPR party-erase (dry-run/`confirm`; journal immutable → `journal_refs_retained`) |

Typiske argumenter: `company` (absolut sti), `actor_kind`, `actor_id`, plus tool-specifikke felter (`entry`, `csv`, `profile`, `confirm_token`, …).

**Journal-flow med MCP**

1. Byg en balanceret `entry` (øre som heltal, ≥2 legs, samme valuta).
2. Valgfrit: `journal_moms_post_suggestion` (`gross_minor` + memo med `#vat25` / `moms:25` / `#moms25`) → net+vat i64-legs til at fylde entry; uden tag → `suggested: false`. **Ingen** auto-post.
3. `journal_post_preview` → gem `confirm_token` + evt. `applied_rules`.
4. `journal_post_commit` med **samme** `entry` + token.
5. Ved fejl: ret entry, ny preview (gammelt token er brugt/ugyldigt).

**Bank / betalinger**

- `generic_dk`: semikolon-CSV.
- `revolut` / `stripe`: **fuld API** (`source: "api"` + env-nøgler). CSV kun som nød/offline (`source: "csv"`).
- Resultatet er **udkast** — post kun via journal preview/commit hvis brugeren beder om det.
- Stripe webhook-kø: `bank_stripe_consume` → valgfrit one-shot `bank_stripe_reconcile_suggest` / `bank_stripe_reconcile_apply_preview` (samme kontrakt som HTTP; se skill `bank-import`).

Env: `KLARBOG_REVOLUT_API_TOKEN`, `KLARBOG_STRIPE_SECRET_KEY` (print aldrig).


### B) HTTP API

Base: `$KLARBOG_API_BASE` (default `http://127.0.0.1:3195`).

**Headers på alle muterende / firmabundne kald:**

```http
Content-Type: application/json
x-klarbog-actor-kind: user
x-klarbog-actor-id: owner
```

(`actor_id` skal matche en actor i firmasts `policy.json`, fx `user:owner`.)

| Metode | Sti | Formål |
|--------|-----|--------|
| GET | `/health` | Health |
| GET | `/api/v1/status` | Status / allowlist |
| POST | `/api/v1/journal/preview` | Fase 1 |
| POST | `/api/v1/journal/commit` | Fase 2 |
| POST | `/api/v1/journal/moms-suggest` | Valgfrit moms-forslag (`gross_minor` + memo `#vat25`; preview only) |
| POST/GET | `/api/v1/crm/parties` | Parter |
| POST/GET | `/api/v1/invoices/drafts` | Fakturakladder |
| PATCH | `/api/v1/invoices/status` | Faktura-status (`draft\|sent\|part_paid\|paid\|void`) |
| POST | `/api/v1/invoices/mark-paid` | Betalt → journalforslag (fuldt beløb) |
| POST | `/api/v1/invoices/mark-part-paid` | Delbetaling (`amount_minor`) → forslag |
| POST | `/api/v1/bank/import/preview` | Bank/API → udkast (`source` + `provider`) |
| POST | `/api/v1/bank/reconcile/suggest` | Match banklinjer ↔ åbne fakturaer |
| POST | `/api/v1/bank/reconcile/apply` | Anvend match → journalforslag; valgfri `preview:true` → confirm-token |
| POST | `/api/v1/webhooks/stripe` | Stripe webhook-ingress (HMAC; kø til drafts) |
| POST | `/api/v1/bank/stripe/consume` | Forbrug webhook-kø → bank-udkast (`confirm` fail-closed) |
| POST | `/api/v1/bank/stripe/reconcile-suggest` | Consume (+valgfri persist) → reconcile-forslag (ingen auto-post) |
| POST | `/api/v1/bank/stripe/reconcile-apply-preview` | Consume → unik safe apply + ConfirmStore preview (ingen auto-commit) |
| GET | `/api/v1/revolut/oauth/start` | Revolut OAuth start (auth-URL) |
| POST | `/api/v1/revolut/oauth/callback` | OAuth code → tokens (gemmes lokalt, returnerer ikke secrets) |
| POST | `/api/v1/revolut/oauth/refresh` | Refresh access-token (fail-closed uden refresh) |
| POST/GET/DELETE | `/api/v1/documents` | Bilags-metadata (+ slet objekt) |
| POST/GET/PATCH | `/api/v1/exceptions` | Undtagelser |
| GET | `/api/v1/retention` | Retention |
| POST | `/api/v1/backup` | Backup-manifest |
| POST | `/api/v1/retention/purge` | Purge lukkede undtagelser (dry-run / `confirm`) |
| POST | `/api/v1/gdpr-export` | GDPR-eksport v1 (firma-scope metadata + note, DEV) |
| POST | `/api/v1/gdpr/erase-party` | GDPR partysletning (dry-run default; `confirm` anonymiserer; journal urørt) |

Svar er typisk et **Envelope**: `{ "ok": true|false, "data": …, "errors": [], "applied_rules": [] }`.

**Eksempel — journal preview**

```bash
curl -sS -X POST "$KLARBOG_API_BASE/api/v1/journal/preview" \
  -H "Content-Type: application/json" \
  -H "x-klarbog-actor-kind: user" \
  -H "x-klarbog-actor-id: owner" \
  -d "{
    \"company\": \"$KLARBOG_COMPANY\",
    \"entry\": {
      \"as_of\": \"2026-08-20T12:00:00Z\",
      \"memo\": \"Kontorartikler\",
      \"actor\": { \"kind\": \"user\", \"id\": \"owner\" },
      \"legs\": [
        { \"account\": \"6000\", \"direction\": \"debit\",  \"amount\": 12500, \"currency\": \"DKK\" },
        { \"account\": \"5800\", \"direction\": \"credit\", \"amount\": 12500, \"currency\": \"DKK\" }
      ]
    }
  }"
```

Beløb `12500` = 125,00 DKK i øre. Brug samme JSON + `confirm_token` mod `/api/v1/journal/commit`.

**Eksempel — bank preview (Revolut API)**

```bash
curl -sS -X POST "$KLARBOG_API_BASE/api/v1/bank/import/preview" \
  -H "Content-Type: application/json" \
  -H "x-klarbog-actor-kind: user" \
  -H "x-klarbog-actor-id: owner" \
  -d "{
    \"company\": \"$KLARBOG_COMPANY\",
    \"source\": \"api\",
    \"provider\": \"revolut\",
    \"currency\": \"DKK\"
  }"
```

(Kræver `KLARBOG_REVOLUT_API_TOKEN` i API-processens miljø. CSV-nød: `"source": "csv"` + `"csv": "..."`.)

**Eksempel — bank preview (Stripe API)**

```bash
curl -sS -X POST "$KLARBOG_API_BASE/api/v1/bank/import/preview" \
  -H "Content-Type: application/json" \
  -H "x-klarbog-actor-kind: user" \
  -H "x-klarbog-actor-id: owner" \
  -d "{
    \"company\": \"$KLARBOG_COMPANY\",
    \"source\": \"api\",
    \"provider\": \"stripe\",
    \"currency\": \"DKK\"
  }"
```

(Kræver `KLARBOG_STRIPE_SECRET_KEY` i API-processens miljø.)

### C) CLI

Når binæren `klarbog` er i PATH:

```bash
klarbog health
klarbog init --company "$KLARBOG_COMPANY" --name "Min ApS" --actor owner
klarbog demo
klarbog retention --company "$KLARBOG_COMPANY"
klarbog backup --company "$KLARBOG_COMPANY"
klarbog gdpr-export --company "$KLARBOG_COMPANY"
klarbog gdpr-erase-party --company "$KLARBOG_COMPANY" --party-id <id>   # dry-run; add --confirm
```

---

## Domæne — kort

- **CRM:** parter i `parties.json`; ledger bruger kun `party_id` (ingen journal-write fra CRM-plugin).
- **Faktura:** kladder + betalings-ledger (remaining = total − summerede delbetalinger); `mark-part-paid` / `mark-paid` giver journalforslag med `party_id` — post via journal to-fase.
- **Dokumenter:** metadata + `path_hint` (relativ, ingen `..`); binære filer via object store.
- **Lagring:** default lokal disk under firmaet; valgfrit **Cloudflare R2 (EU)** via `KLARBOG_STORAGE=r2` og `KLARBOG_R2_*` (fail-closed uden for EU).
- **Regler (DK-dev):** memo påkrævet, kontonummer kun cifre, **chart stub** (`1000` bank, `1500` AR, `4400` AP, udgift `4000`–`6999`; bank-CSV ofte `5800`/`6100`), valgfri hint `dk.bookkeeping.known_account` (blokerer ikke), moms-hint `dk.vat.rate` fra memo (`vat:25`/`moms:0`/`25%`), `dk.vat.split_hint` (i64+bps, 25%/0), `dk.expense.receipt_required` blokerer udgiftsdebet (4000–6999) uden `party_id` og uden `#receipt`/`document_id:` i memo; `dk.expense.receipt_hint` når `party_id` findes men receipt-signal mangler.

---

## Adfærd over for brugeren

- Tal **dansk**, medmindre brugeren beder om andet.
- Forklar poster i menneskesprog (konto + beløb i kroner), men send **øre som heltal** til API/MCP.
- Vis altid hvad der vil blive bogført **før** commit; vent på eksplicit accept til fase 2.
- Ved `ok: false`: læs `errors`, ret, kør ny preview.
- Rør ikke andre systemer (bank-login, skat.dk, e-mail) medmindre brugeren eksplicit beder om det og produktet understøtter det.

---

## Når du er klar

Sig til brugeren (tilpas stier efter deres install):

```
┌─ Klarbog AI-klar ────────────────────────────────────┐
│  ✓ Denne prompt er fulgt                             │
│  ✓ Health/API eller MCP kan nås                      │
│  ✓ Journal = preview → commit                        │
│  ✓ Beløb = øre (heltal)                              │
│                                                      │
│  Klar til at hjælpe med bogføring på din installation│
└──────────────────────────────────────────────────────┘
```

---

## Ægthed

Denne fil shippes med Klarbog-installationen som:

`docs/agent-setup/prompt.md`

(eller tilsvarende sti i jeres pakke). Mønster inspireret af Cloudflare’s agent-setup-prompt (`https://developers.cloudflare.com/agent-setup/prompt.md`), men **formålet her er produkt-snak**, ikke Cloudflare-udviklermiljø.

Internt udvikler-bootstrap (repo, verify-gate) hører **ikke** hjemme i denne fil — se `DEVELOPER.md`.
