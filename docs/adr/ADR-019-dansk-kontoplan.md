# ADR-019 — Dansk standardkontoplan v1 (chart restructure)

Proposed (2026-08-21 — owner requested design; implementation awaits owner GO)

## Context

The DEV chart stub ([ADR-011](ADR-011-moms-suggest.md) era) is internally
inconsistent, discovered while adding ledger read views:

| Problem | Detail |
|---|---|
| Revenue inside expense band | `InvoiceConfig.revenue_account = "6100"` and `BankImportConfig.income_account = "6100"` sit inside the expense band 4000–6999 (`is_expense_account_code`). A P&L grouping on current bands would count revenue as expense. |
| Two bank accounts | Invoice plugin books bank as `1000`; bank plugin books bank as `5800`. The same real-world account has two codes, so bank saldo splits across two rows. |
| Bank inside expense band | `5800` ∈ 4000–6999, so bank **debits** (money in) are subject to the fail-closed expense receipt rule (`dk.expense.receipt_required`). Flows only pass today because legs happen to carry `party_id`. Wrong semantics that accidentally validates. |
| AP inside expense band | `4400` (accounts payable) also sits inside 4000–6999. |

Current code touchpoints (survey 2026-08-21): `klarbog-plugin-rules-dk`
(`chart.rs` constants + bands, receipt rule in `lib.rs`), `InvoiceConfig`
(draft.rs), `BankImportConfig` (map.rs), UI journal form defaults
(6000/5800), CLI `smoke-post`, MCP journal tool, `core::journal_ops` and
plugin test fixtures, `rules_chart` HTTP/MCP read surfaces, ui-smoke.

## Decision (target chart)

Adopt the common Danish SMB standardkontoplan structure (e-conomic-style
numbering). Bands are disjoint; every plugin default gets one canonical code.

### Bands

| Band | Meaning | Type |
|---|---|---|
| 1000–1999 | Omsætning (revenue) | Resultat |
| 2000–2999 | Vareforbrug / direkte omkostninger | Resultat (receipt-gated) |
| 3000–3999 | Personaleomkostninger | Resultat (NOT receipt-gated) |
| 4000–4999 | Kapacitetsomkostninger (drift) | Resultat (receipt-gated) |
| 5000–5999 | Aktiver | Balance |
| 6000–6999 | Passiver, gæld og moms | Balance |

### Named defaults (canonical codes)

| Code | Label | Replaces |
|---|---|---|
| 1010 | Salg af varer/ydelser | 6100 (revenue/income) |
| 2010 | Vareforbrug | — (new) |
| 4010 | Driftsomkostninger | 6000 (default expense) |
| 5600 | Debitorer (AR) | 1500 |
| 5810 | Kasse | — (new, band only) |
| 5820 | Bank | 1000 **and** 5800 (unified) |
| 6840 | Kreditorer (AP) | 4400 |
| 6901 | Salgsmoms (udgående) | — (reserved) |
| 6902 | Købsmoms (indgående) | — (reserved) |
| 6903 | Momsafregning | — (reserved) |

6901–6903 are **reserved** for a future VAT-legs feature; `moms_post_suggestion`
stays a hint-only splitter until then.

### Rule changes (`klarbog-plugin-rules-dk`)

- `is_expense_account_code` → true for 2000–2999 **or** 4000–4999 (personale
  3000–3999 exempt: salaries have no receipts; band renamed accordingly).
- Receipt rule (`dk.expense.receipt_required`) unchanged in behavior but now
  fires on the corrected bands — bank/AP/revenue debits no longer touch it.
- `is_known_dk_account` allowlist = named defaults above + all six bands
  (still hint-only via `RULE_KNOWN_ACCOUNT`; digits-only hard-fail unchanged).
- `chart_stub_entries` lists the six bands + named defaults with Danish labels
  (feeds `/api/v1/rules/chart`, MCP tool, and the SSR Kontoplan page).

### Plugin/config defaults

- `InvoiceConfig`: ar 5600 · revenue 1010 · ap 6840 · expense 4010 · bank 5820.
- `BankImportConfig`: expense 4010 · bank 5820 · income 1010.
- UI journal form defaults: 4010 debit / 5820 credit.
- CLI smoke-post, MCP journal tool, core/plugin test fixtures follow.

## Migration

None — deliberately. The journal is immutable (ADR-004) and this product is
DEV-isolated (ADR-003/ADR-014, no production data exists). Existing dev
companies keep their history; old codes remain valid digit accounts and are
merely flagged by the hint-only `RULE_KNOWN_ACCOUNT`. Demo/dev companies are
recreated with the new chart. No remap table, no reversal migration.

Residual risk (accepted, DEV): mixed-chart history in old dev companies shows
old codes without band labels; the receipt rule no longer gates legacy 6000
debits. Recreate the company to get coherent data.

## Implementation plan (slices, each VERIFY_OK + ui-smoke green)

1. **rules-dk**: new constants/bands/labels, `is_expense_account_code`,
   `is_known_dk_account`, `chart_stub_entries`, unit tests (incl. negative:
   bank debit no longer receipt-gated; 3000-band exempt).
2. **Plugins**: `InvoiceConfig` + `BankImportConfig` defaults + their tests.
3. **Surfaces**: UI journal defaults, CLI smoke-post, MCP journal tool,
   core/plugin fixtures, ui-smoke account assertions (incl. new negative
   check: expense debit on 4010 without receipt fails closed).
4. **Docs**: this ADR → Accepted; ADR-018 pattern note unchanged.

Slices 1–3 must land back-to-back on `rust-dev` (mixed defaults across slices
would break the cross-plugin smoke); the branch CI gate covers each push.

## Consequences

- Bank saldo becomes one row (5820); parties' AR moves to 5600; P&L grouping
  by band becomes possible (1000s revenue vs 2000s/4000s costs) — unblocks a
  future resultatopgørelse view.
- The receipt fail-closed rule finally matches its intent: only real expense
  debits require receipt/party.
- Any tooling that hardcoded 6000/5800 (agents, docs, saved smoke payloads)
  must move to 4010/5820; the old codes stay postable but are flagged unknown.
