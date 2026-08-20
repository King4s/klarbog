# Klarbog Swarm — KANONISK slice-graf (eneste nummerering)

0. Fork + Rust scaffold + swarm/ + ADR-001..005 (CRITICAL landings)
1. Money (i64) + journal (mandatory double-entry) + actor + sqlite WAL/FK/migrations
2. API + MCP + two-phase confirm + Actor/company AuthZ
3. Plugin-host (compile-time traits) + rules-dk
4. CRM plugin (no journal-write dependency)
5. Bank CSV (GenericDk) + **Revolut API** (ADR-008) + **Stripe API** (ADR-009); CSV only offline fallback for those rails
6. Invoice + party_id
7. Documents + exceptions (**+ Cloudflare R2 EU ObjectStore**, ADR-007)
8. Skills + agent-demo
9. Backup / GDPR / retention / templates (reuse ObjectStore; EU residency)

Owner add-ons: Cloudflare EU (R2); Revolut **API**; Stripe **API**.

10. Payments hardening: Stripe webhooks + Revolut OAuth
11. rules-dk expansion (VAT/account/receipt hints)
12. Bank reconcile suggestions + invoice lifecycle states
13. R2 DeleteObject + retention purge enforce
14. Install/release packaging docs

## Wave 2 — harden + wire (landed)

15. Revolut OAuth **refresh_token** rotate + fail-closed expiry
16. Stripe webhook **queue consumer** → bank drafts
17. Bank reconcile **apply** → journal preview suggestion
18. GDPR export expand (company-scoped v1 metadata)
19. rules-dk: expense without receipt **blocks commit**
20. Invoice **part_paid** payment suggestion
21. Soft linegate hygiene

## Wave 3 — productize (landed)

22. MCP parity for wave2 HTTP (consume, reconcile apply, mark-part-paid, oauth refresh) ✅
23. Invoice **payment ledger** — remaining balance for part_paid / mark-paid
24. Stripe consume rows → reconcile **suggest** helper (one-shot pipeline, still no auto post)
25. GDPR **party erasure** dry-run/confirm (metadata + optional R2 delete; keep journal immutable)
26. rules-dk: VAT **split suggestion** (i64 minor + bps; no f64) for moms 25/0
27. Reconcile apply optional `preview: true` → ConfirmStore token (wire to journal commit)
28. `klarbog demo` covers wave2/3 surfaces (drop Revolut skip TODO)
29. CHANGELOG + agent-setup prompt refresh for wave3

## Wave 4 — deepen (after wave3)

30. MCP `gdpr_erase_party` + retention skill note (journal immutable)
31. One-shot Stripe consume → reconcile apply preview (opt-in, ConfirmStore)
32. rules-dk: account range ↔ chart stub (DK expense/bank/AR codes documented)
33. Backup manifest includes payments[] + erase audit trail file
34. Soft linegate re-check after wave3 growth; split any ≥300
35. Offline “contract smoke” script for new HTTP routes (no network) ✅

## Wave 5 — polish (landed)

36. Soft-split `backup.rs` under 300; keep erase_audit lean ✅
37. MCP tools for `stripe/reconcile-suggest` + `stripe/reconcile-apply-preview`
38. ADR: DK bogføring retention (5y) vs GDPR erase — journal never purged by party erase
39. `contract-smoke` extend: oauth refresh fail-closed + mark-paid remaining ✅
40. Bank reconcile force path MCP + skill cross-links ✅ (light: skill cross-link)
41. CHANGELOG/prompt sync for wave4 endpoints ✅

Advance only after verifier green. Presence of `swarm/STOP` halts dispatch.

## Wave 6 — next product gaps (landed)

42. Soft-split `klarbog-mcp/src/tools/mod.rs` under 300
43. Invoice/journal: moms post **suggestion** from `split_vat25_inclusive` (preview only) ✅
44. Multi-currency fail-closed regression tests (batch currency mismatch) ✅
45. `package-dev.sh` + INSTALL: contract-smoke + dist checksums note ✅
46. Exception close-on-apply coverage for stripe apply-preview path ✅

Advance only after verifier green. Presence of `swarm/STOP` halts dispatch.

## Wave 7 — hygiene (landed)

47. Soft-split `contract_smoke` under 300; moms-suggest docs polish ✅

## Wave 8 — contract deepen (landed)

48. `contract-smoke` extend: moms-suggest i64 legs + multi-currency import preview **400** ✅

## Wave 9 — moms-suggest ADR/skill (landed)

49. ADR-011 + dedicated `docs/skills/moms-suggest.md` (preview-only i64; no auto-post) ✅

## Wave 10 — bank/oauth fail-closed tests (landed)

50. Strengthen fail-closed bank/oauth: blank refresh, provider HTTP error preserves secrets, expired-without-refresh resolve, Stripe API import missing env (HTTP + contract-smoke) ✅

## Wave 11 — demo moms-suggest (landed)

51. `klarbog demo` covers `moms_post_suggestion` (i64 `#vat25` legs + optional-none; never auto-posts) ✅

## Wave 12 — scratch-marker gitignore (landed)

52. Ignore autonomous scratch `/wave*.md`, `docs/status-*.md`, `/status/` (canonical `STATUS.md` / `docs/status.md` kept) ✅

## Wave 13 — packaging env polish (landed)

53. Replace upstream-shaped `.env.example` with Klarbog `KLARBOG_*` template; link from README / INSTALL / DEVELOPER (no sister-product env names, no absolute host paths) ✅

## Wave 14 — MCP docs sync (landed)

54. Sync public MCP allowlist docs: `bank_stripe_reconcile_suggest` + `bank_stripe_reconcile_apply_preview` in prompt.md tools table + bank-import skill + INSTALL (parity with registry; no auto journal post) ✅

## Wave 15 — idle hardening (landed)

55. Thin negative-path: `POST /api/v1/journal/moms-suggest` with negative `gross_minor` → **400** (HTTP maps `MomsPostSuggestionError`; i64; never suggests) ✅

## Wave 16 — idle hardening (landed)

56. Thin negative-path: `POST /api/v1/journal/moms-suggest` with unsupported memo VAT (`vat:12`) → **400** (HTTP maps `MomsPostSuggestionError::Memo`; i64 gross untouched; never suggests) ✅

## Wave 17 — MCP moms-suggest fail-closed (landed)

57. MCP parity: `journal_moms_post_suggestion` with unsupported memo VAT (`vat:12`) → envelope **not ok** (fail-closed; i64 gross untouched; never suggests) ✅

## Wave 18 — MCP moms-suggest negative gross (landed)

58. MCP parity: `journal_moms_post_suggestion` with negative `gross_minor` → envelope **not ok** (fail-closed; i64; never suggests) ✅

## Wave 19 — MCP retention purge (landed)

59. MCP parity: `retention_purge` mirrors `POST /api/v1/retention/purge` (dry-run/`confirm`; optional `gc_orphan_documents`; journal untouched) ✅

## Wave 20 — MCP bank reconcile suggest (landed)

60. MCP parity: `bank_reconcile_suggest` mirrors `POST /api/v1/bank/reconcile/suggest` (`rows` or CSV/provider; `raise_exceptions`; no journal post) ✅

## Wave 21 — MCP GDPR export (landed)

61. MCP parity: `gdpr_export` mirrors `POST /api/v1/gdpr-export` (company-scoped metadata → `gdpr_export.json`; i64 invoice totals; no binary blobs) ✅

Advance only after verifier green. Presence of `swarm/STOP` halts dispatch.
