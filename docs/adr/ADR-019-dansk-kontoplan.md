# ADR-019 — Kontoplan: adopt the original chart (rev. 2)

Accepted (2026-08-21 — owner GO «implementér»; Phase 1 landed on rust-dev:
typed 48-account chart in rules-dk, type-derived rules, plugin defaults,
all surfaces + fixtures remapped, verify + ui-smoke green)

## Context

The Rust DEV chart stub is internally inconsistent, discovered while adding
ledger read views:

| Problem | Detail |
|---|---|
| Revenue inside expense band | `InvoiceConfig.revenue_account = "6100"` and `BankImportConfig.income_account = "6100"` sit inside the expense band 4000–6999 (`is_expense_account_code`). |
| Two bank accounts | Invoice plugin books bank as `1000`; bank plugin books bank as `5800`. Bank saldo splits across two rows. |
| Bank inside expense band | `5800` ∈ 4000–6999, so bank **debits** (money in) hit the fail-closed expense receipt rule. Flows only pass because legs happen to carry `party_id`. |
| AP inside expense band | `4400` (accounts payable) also sits inside 4000–6999. |

**Original-project comparison (rev. 2 finding).** The original TS codebase on
`main` does NOT have this problem — it solved it properly:

- `seedAccounts` (src/core/ledger.ts) seeds ~50 Danish accounts, each with
  explicit metadata: `type` (income/expense/asset/liability/equity/vat),
  `normal_balance`, `default_vat_code`. Semantics come from **metadata, not
  numeric bands**.
- `seedNativeAccountRoles` (src/core/account-roles.ts) adds a **role
  indirection**: booking code resolves semantic roles → account numbers via an
  audited `account_role_mappings` table (proposals, explicit confirmation,
  compatibility checks). Nothing hardcodes numbers.
- Native numbering: 1xxx income + debitorer (1100) + salgsmoms (1200),
  2000 Bank, 3xxx driftsomkostninger + staff (3500–3599), 4000 Købsmoms,
  4500 Momsafregning, 5xxx equity + anlæg, 7xxx kortfristet gæld
  (7000 kreditorer).

The Rust stub invented a **conflicting** numbering — worst case `1000`, which
means *Bank* in the Rust stub but *Omsætning, ydelser* in the original. Any
data or agent tooling that crosses the two worlds would misbook revenue as
bank. Rev. 1 of this ADR proposed e-conomic-style numbering; that would have
added a *third* incompatible scheme and is withdrawn.

## Decision

Adopt the **original chart** as the single source of truth for numbering and
semantics. Two phases:

### Phase 1 — static chart parity (rust-dev)

Replace the band stub in `klarbog-plugin-rules-dk` with a static table
mirroring the original `seedAccounts` rows (number, Danish label, type,
normal balance). Derive all rule semantics from account `type`:

- `is_expense_account_code(n)` → lookup `type == expense`; the receipt rule
  (`dk.expense.receipt_required`) fires on expense debits **except** staff
  accounts 3500–3599 (salaries have no receipts) and 5820 Afskrivninger
  (non-cash internal booking — no receipt exists).
- `is_known_dk_account` → membership in the table (still hint-only via
  `RULE_KNOWN_ACCOUNT`; digits-only hard-fail unchanged).
- `chart_stub_entries` → the real table (feeds `/api/v1/rules/chart`, MCP,
  and the SSR Kontoplan page, which then shows label + type per account).

Plugin defaults move to the original's native role numbers:

| Config | Old (stub) | New (original) |
|---|---|---|
| Invoice `ar_account` | 1500 | 1100 Debitorer |
| Invoice `revenue_account` | 6100 | 1000 Omsætning, ydelser |
| Invoice `ap_account` | 4400 | 7000 Leverandørgæld |
| Invoice/Bank `bank_account` | 1000 / 5800 | 2000 Bank |
| Invoice/Bank `expense_account` | 6000 | 3000 (operational_default) |
| Bank `income_account` | 6100 | 1000 |
| UI journal defaults | 6000/5800 | 3000/2000 |

VAT accounts 1200 (salgsmoms), 4000 (købsmoms), 4500 (momsafregning) become
known accounts; `moms_post_suggestion` stays hint-only until a VAT-legs
feature books against them.

### Phase 2 — role indirection (later, own ADR)

Port the original's `account_role_mappings` concept (roles: bank, debtors,
creditors, output_vat, input_vat, vat_settlement, operational_default) so
plugins resolve roles instead of constants, with audited explicit
confirmation for imported charts. Out of scope here; Phase 1 keeps plugin
config structs but fixes their defaults.

## Migration

None — deliberately. The journal is immutable (ADR-004) and the Rust product
is DEV-isolated (ADR-003/ADR-014, no production data). Existing dev companies
keep their history; old stub codes remain postable digit accounts flagged by
the hint-only known-account rule. Demo/dev companies are recreated.

Residual risk (accepted, DEV): mixed-chart history in old dev companies; the
receipt rule no longer gates legacy 6000 debits. Recreate to get coherent data.

## Implementation plan (slices, each VERIFY_OK + ui-smoke green)

1. **rules-dk**: static account table (number/label/type/normal-balance) from
   the original seed, type-derived predicates, staff exemption, unit tests
   (incl. negative: 2000 bank debit not receipt-gated; 3500 løn exempt;
   1000 credit books as revenue not bank).
2. **Plugins**: `InvoiceConfig` + `BankImportConfig` defaults + tests.
3. **Surfaces**: UI journal defaults, CLI smoke-post, MCP journal tool,
   core/plugin fixtures, ui-smoke account assertions (new negative check:
   expense debit on 3000 without receipt fails closed).
4. **Docs**: this ADR → Accepted; Kontoplan page gains type column.

Slices 1–3 land back-to-back on `rust-dev` (mixed defaults across pushes
would break the cross-plugin smoke); branch CI gates each push.

## Consequences

- Rust port and original agree on what every account number means; agent
  tooling and fixtures port across without renumbering.
- One bank row (2000), debitor-saldi on 1100, and P&L grouping via account
  `type` — unblocks the resultatopgørelse view without band guesswork.
- The receipt fail-closed rule matches intent: only real expense debits
  (minus staff) require receipt/party.
- The Kontoplan SSR page upgrades from 4 stub rows to the real chart with
  Danish labels.
