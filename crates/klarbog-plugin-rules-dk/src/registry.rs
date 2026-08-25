//! Regelregister — porteret fra originalens `rules/dk/*.yaml` (ejerordre
//! 2026-08-21: originalen er facit). Hver post svarer på originalens fem
//! spørgsmål: hvad kræver reglen (name), hvornår gælder den (enforced_by),
//! hvad afvises (severity hard_stop), hvilken kilde (source_id + §),
//! og hvilken test beviser det (proven_by).
//!
//! Kun regler porten FAKTISK håndhæver optages; det originalen kræver ud
//! over portens håndhævelse står ærligt i `gaps` (ingen selv-attestering).

/// Provenance: rule ids, names, sources and § refs are copied verbatim from
/// the original project's rule library at this commit.
pub const REGISTRY_BASIS: &str = "origin/main 553c204 rules/dk/{bookkeeping,invoices}.yaml";

#[derive(Debug, Clone, Copy)]
pub struct RegisteredRule {
    pub rule_id: &'static str,
    pub name: &'static str,
    pub source_id: &'static str,
    /// Lovbestemmelser (§-referencer) fra originalens provisions-liste.
    pub provisions: &'static [&'static str],
    pub severity: &'static str,
    /// Hvor i porten reglen håndhæves.
    pub enforced_by: &'static str,
    /// Tests der beviser håndhævelsen (crate::modul::testnavn).
    pub proven_by: &'static [&'static str],
    /// Originalens machine_rule-krav som porten endnu ikke opfylder.
    pub gaps: &'static [&'static str],
}

pub fn registered_rules() -> &'static [RegisteredRule] {
    &[
        RegisteredRule {
            rule_id: "DK-BOOKKEEPING-BALANCED-001",
            name: "Double-entry postings must balance",
            source_id: "DK-BOGFORINGSLOVEN-2022-700",
            provisions: &["§ 7, stk. 1"],
            severity: "hard_stop",
            enforced_by: "klarbog-journal JournalEntry::validate (balance pr. valuta, afvist før post)",
            proven_by: &["klarbog-journal entry::tests::rejects_unbalanced"],
            gaps: &[],
        },
        RegisteredRule {
            rule_id: "DK-BOOKKEEPING-APPEND-ONLY-001",
            name: "Posted journal entries are append-only",
            source_id: "DK-BOGFORINGSLOVEN-2022-700",
            provisions: &["§ 9, stk. 3", "§ 13, stk. 1"],
            severity: "hard_stop",
            enforced_by: "klarbog-store-sqlite: kun INSERT med digest/prev_digest-kæde; ingen update/delete-API",
            proven_by: &["klarbog-store-sqlite open::tests::wal_fk_and_balanced_append"],
            gaps: &[],
        },
        RegisteredRule {
            rule_id: "DK-BOOKKEEPING-REVERSAL-001",
            name: "Corrections must create a single traceable reversal entry",
            source_id: "DK-BOGFORINGSLOVEN-2022-700",
            provisions: &["§ 9, stk. 3"],
            severity: "hard_stop",
            enforced_by: "JournalEntry::reversal (eksakt negation, samme konti) + to-faset UI reverse_preview/commit",
            proven_by: &[
                "klarbog-journal entry::tests::accepts_balanced_and_reverses",
                "klarbog-api ui::tests::journal_flow::ui_journal_preview_then_commit_posts",
            ],
            gaps: &[],
        },
        RegisteredRule {
            rule_id: "DK-BOOKKEEPING-DOCUMENT-001",
            name: "Bookkeeping entries require evidence where relevant",
            source_id: "DK-BOGFORINGSLOVEN-2022-700",
            provisions: &["§ 9, stk. 1", "§ 9, stk. 2"],
            severity: "hard_stop",
            enforced_by: "rules-dk: udgiftsdebiteringer kræver bilagssignal (#receipt/doc) — fail-closed i preview",
            proven_by: &[
                "klarbog-plugin-rules-dk rules_tests::blocks_expense_without_party_or_receipt",
                "klarbog-plugin-rules-dk rules_tests::allows_expense_with_receipt_tag",
            ],
            gaps: &[],
        },
        RegisteredRule {
            rule_id: "DK-BOOKKEEPING-RETENTION-001",
            name: "Supporting material must carry a retention deadline from the end of the fiscal year plus five years",
            source_id: "DK-BOGFORINGSLOVEN-2022-700",
            provisions: &["§ 12, stk. 1"],
            severity: "hard_stop",
            enforced_by: "retention-plugin: retain_until på dokument/journal/bank ved oprettelse + retention-status-rapport (CLI: retention-status); fiscalYear* fra policy.json; global retain_days + purge dry-run→confirm",
            proven_by: &[
                "klarbog-types retention_deadline::tests",
                "klarbog-plugin-retention status_report::tests::report_counts_objects_and_expiry_after_deadline",
                "klarbog-plugin-bank store::tests::commit_assigns_batch_hash_and_skips_duplicates",
            ],
            gaps: &[],
        },
        RegisteredRule {
            rule_id: "DK-BOOKKEEPING-BANK-IMPORT-001",
            name: "Imported bank transactions must preserve transaction date, amount, text, and traceable source batch",
            source_id: "DK-BOGFORINGSLOVEN-2022-700",
            provisions: &["§ 7, stk. 1", "§ 9, stk. 2"],
            severity: "hard_stop",
            enforced_by: "bank-plugin: commit_import persisterer dato/tekst/beløb med import_batch_id, source_file_hash og deterministisk transaction_hash; dubletter sprunget over (forbid duplicate fingerprint)",
            proven_by: &[
                "klarbog-plugin-bank store::tests::commit_assigns_batch_hash_and_skips_duplicates",
                "klarbog-plugin-bank batch::tests::identical_rows_get_distinct_occurrence_fingerprints",
            ],
            gaps: &[],
        },
        RegisteredRule {
            rule_id: "DK-BOOKKEEPING-RECONCILIATION-001",
            name: "Bank reconciliation must show matched and unmatched imported transactions for a period",
            source_id: "DK-BOGFORINGSLOVEN-2022-700",
            provisions: &["§ 11, stk. 1", "§ 11, stk. 2"],
            severity: "hard_stop",
            enforced_by: "bank-plugin: reconcile suggest/apply med confidence-tærskel (fail-closed under 5000 bps uden force) + periode-rapport over matchede/umatchede (UI: Bank → Afstemningsrapport)",
            proven_by: &[
                "klarbog-plugin-bank reconcile_tests",
                "klarbog-plugin-bank reconcile_report::tests::splits_matched_and_unmatched_with_totals",
            ],
            gaps: &[],
        },
        RegisteredRule {
            rule_id: "DK-CREDIT-NOTE-001",
            name: "Credit notes must reference the original issued invoice and mirror the corrected VAT effect",
            source_id: "DK-MOMSBEKENDTGORELSEN-2023-1435",
            provisions: &["§ 58, stk. 2", "§ 66, stk. 1, nr. 6"],
            severity: "hard_stop",
            enforced_by: "invoice-plugin: fortløbende CN-nr pr. regnskabsår (auto eller manuelt med scope-validering); del- og fuld kreditnota med kumulativt loft; memo invoice:{id}:credit:{CN} · {reason}; immutable JSON-dokument (sha256, retain_until) under invoices/issued/",
            proven_by: &[
                "klarbog-plugin-invoice credit::tests::full_credit_negates_send_booking_incl_vat",
                "klarbog-plugin-invoice credit::tests::partial_then_residual_lands_exactly_on_original",
                "klarbog-plugin-invoice credit::tests::cumulative_cap_rejects_over_credit",
                "klarbog-plugin-invoice sequences::tests::reserve_is_fail_closed_on_race",
                "klarbog-plugin-invoice sequences::tests::manual_scope_mismatch_is_rejected",
                "klarbog-plugin-documents credit_note::tests::attach_writes_immutable_json_with_sha256",
            ],
            gaps: &[],
        },
        RegisteredRule {
            rule_id: "DK-INVOICE-DUE-DATE-001",
            name: "Customer invoices must expose a deterministic due date and overdue classification",
            source_id: "DK-RENTELOVEN-2014-459",
            provisions: &["§ 3, stk. 1", "§ 3, stk. 2"],
            severity: "hard_stop",
            enforced_by: "invoice-plugin: issue_date + due_date på send; effective_due (eksplicit eller +30 d); overdue_days kun ved positivt inddriveligt hovedstol; UI-kolonne Forfald",
            proven_by: &[
                "klarbog-plugin-invoice due_date::tests",
                "klarbog-plugin-invoice issue::tests::record_issue_sets_dates_and_number",
                "klarbog-plugin-invoice issue::tests::record_issue_uses_party_payment_terms",
                "klarbog-plugin-crm payment_terms::tests",
                "klarbog-plugin-invoice reminders_tests::registers_statutory_reminder_fee_on_overdue_invoice",
                "klarbog-plugin-invoice late_compensation_tests::claim_open_balance_includes_compensation",
            ],
            gaps: &[],
        },
        RegisteredRule {
            rule_id: "DK-INVOICE-REMINDER-FEE-001",
            name: "Overdue customer invoices may register statutory reminder fees with deterministic limits",
            source_id: "DK-RENTELOVEN-2014-459",
            provisions: &["§ 5, stk. 1"],
            severity: "hard_stop",
            enforced_by: "invoice-plugin reminders: max 100 DKK; max 3 per claim; min 10 dage mellem; overdue + positivt inddriveligt hovedstol; DKK only; sent/part_paid",
            proven_by: &[
                "klarbog-plugin-invoice reminders_tests::registers_statutory_reminder_fee_on_overdue_invoice",
                "klarbog-plugin-invoice reminders_tests::blocks_fourth_reminder",
                "klarbog-plugin-invoice reminders_tests::blocks_reminder_sent_too_soon",
            ],
            gaps: &[
                "SQLite invoice_reminders + audit_log rækker",
                "BEGIN IMMEDIATE concurrency på tværs af processer",
            ],
        },
        RegisteredRule {
            rule_id: "DK-INVOICE-REMINDER-FEE-BOOKKEEPING-001",
            name: "Registered reminder fees must be bookable once to receivables and non-VAT claim income",
            source_id: "DK-BOGFORINGSLOVEN-2022-700",
            provisions: &["§ 9, stk. 1"],
            severity: "hard_stop",
            enforced_by: "invoice-plugin: reminder_post_journal_suggestion (AR debet / 1010 kredit); to-faset UI reminder_post_preview/commit; posted_journal_id fail-closed",
            proven_by: &[
                "klarbog-plugin-invoice reminders_tests::posts_reminder_once_and_rejects_double_post",
                "klarbog-api ui::tests::invoice_flow::ui_reminder_register_and_post",
            ],
            gaps: &[
                "invoice_reminder_postings append-only link-tabel",
                "accountRoleCompatibility / resolveClaimIncomeAccount",
                "bogføring af specifikt reminder_id når flere unposted",
            ],
        },
        RegisteredRule {
            rule_id: "DK-INVOICE-LATE-INTEREST-001",
            name: "Overdue customer invoices must support deterministic statutory late-interest calculation",
            source_id: "DK-RENTELOVEN-2014-459",
            provisions: &["§ 5, stk. 1"],
            severity: "hard_stop",
            enforced_by: "invoice-plugin late_interest: morarente = referencesats + 8 pct; halvårlig tabel; datobevidst hovedstol; inkrementelt krav siden sidste claim",
            proven_by: &[
                "klarbog-plugin-invoice late_interest_tests::calculates_overdue_partial_payment",
                "klarbog-plugin-invoice late_interest_tests::cumulative_interest_matches_reference_case",
                "klarbog-plugin-invoice late_interest_tests::staged_claims_bill_incrementally",
                "klarbog-plugin-invoice late_interest_tests::defaults_to_statutory_table",
                "klarbog-mcp invoice_settlement::tests::mcp_interest_calc_overdue",
            ],
            gaps: &[
                "proposeInterestCorrection / postInterestCorrection (over-claimed morarente)",
                "invoice_interest_corrections og evidence-plan tabeller",
                "referencesats før 2023-01-01 (kræver eksplicit rate)",
                "payment_date på betalinger (port bruger unix_ms→UTC-dato)",
                "rente-af-rente / compound interest",
            ],
        },
        RegisteredRule {
            rule_id: "DK-INVOICE-LATE-INTEREST-REGISTER-001",
            name: "A late-interest claim may only be registered from a deterministic calculation and must remain traceable in the claim balance",
            source_id: "DK-RENTELOVEN-2014-459",
            provisions: &["§ 5, stk. 1"],
            severity: "hard_stop",
            enforced_by: "invoice-plugin: register_late_interest persisterer immutable claim i invoices.json; duplikat (dato+rate) afvist; claim_open_balance_minor",
            proven_by: &[
                "klarbog-plugin-invoice late_interest_tests::register_rejects_duplicate_and_zero_increment",
                "klarbog-plugin-invoice late_interest_tests::staged_claims_bill_incrementally",
                "klarbog-mcp invoice_settlement::tests::mcp_claim_interest_requires_confirm",
                "klarbog-mcp invoice_settlement::tests::mcp_claim_and_post_interest_preview",
            ],
            gaps: &[
                "SQLite invoice_interest_claims + audit_log rækker",
                "BEGIN IMMEDIATE concurrency på tværs af processer",
            ],
        },
        RegisteredRule {
            rule_id: "DK-INVOICE-LATE-INTEREST-BOOKKEEPING-001",
            name: "Registered late-interest claims must be bookable once to receivables and non-VAT claim income",
            source_id: "DK-BOGFORINGSLOVEN-2022-700",
            provisions: &["§ 9, stk. 1"],
            severity: "hard_stop",
            enforced_by: "invoice-plugin: interest_post_journal_suggestion (AR debet / 1010 kredit); to-faset UI interest_post_preview/commit; posted_journal_id fail-closed",
            proven_by: &[
                "klarbog-api ui::tests::invoice_flow::ui_interest_register_and_post",
                "klarbog-mcp invoice_settlement::tests::mcp_claim_and_post_interest_preview",
            ],
            gaps: &[
                "invoice_interest_postings append-only link-tabel",
                "accountRoleCompatibility / resolveClaimIncomeAccount",
                "bogføring af specifikt claim_id når flere unposted",
            ],
        },
        RegisteredRule {
            rule_id: "DK-INVOICE-LATE-COMPENSATION-001",
            name: "Overdue commercial customer invoices must support deterministic statutory fixed compensation",
            source_id: "DK-RENTELOVEN-2014-459",
            provisions: &["§ 9a, stk. 1"],
            severity: "hard_stop",
            enforced_by: "invoice-plugin late_compensation: 310 DKK max; commercial CRM party; overdue + positivt inddriveligt hovedstol; issue_date >= 2013-03-01",
            proven_by: &[
                "klarbog-plugin-invoice late_compensation_tests::commercial_overdue_invoice_is_eligible",
                "klarbog-plugin-invoice late_compensation_tests::private_buyer_is_not_eligible",
                "klarbog-mcp invoice_settlement::tests::mcp_compensation_calc_overdue_commercial",
            ],
            gaps: &[
                "EAN/GLN og publicRecipient som alternativ erhvervsbevis (JUR-15)",
                "buyer.vatOrCvr på issued payload",
            ],
        },
        RegisteredRule {
            rule_id: "DK-INVOICE-LATE-COMPENSATION-REGISTER-001",
            name: "A fixed compensation claim may only be registered once per invoice and must remain traceable in the claim balance",
            source_id: "DK-RENTELOVEN-2014-459",
            provisions: &["§ 9a, stk. 1"],
            severity: "hard_stop",
            enforced_by: "invoice-plugin: register_invoice_compensation persisterer immutable claim i invoices.json; duplikat afvist; claim_open_balance_minor inkl. total_compensation_minor",
            proven_by: &[
                "klarbog-plugin-invoice late_compensation_tests::register_rejects_duplicate_and_private_buyer",
                "klarbog-plugin-invoice late_compensation_tests::claim_open_balance_includes_compensation",
                "klarbog-mcp invoice_settlement::tests::mcp_claim_compensation_requires_confirm",
                "klarbog-mcp invoice_settlement::tests::mcp_claim_and_post_compensation_preview",
            ],
            gaps: &[
                "SQLite invoice_compensation_claims + audit_log rækker",
                "BEGIN IMMEDIATE concurrency på tværs af processer",
            ],
        },
        RegisteredRule {
            rule_id: "DK-INVOICE-LATE-COMPENSATION-BOOKKEEPING-001",
            name: "Registered fixed compensation claims must be bookable once to receivables and non-VAT claim income",
            source_id: "DK-BOGFORINGSLOVEN-2022-700",
            provisions: &["§ 9, stk. 1"],
            severity: "hard_stop",
            enforced_by: "invoice-plugin: compensation_post_journal_suggestion (AR debet / 1010 kredit); to-faset UI compensation_post_preview/commit; posted_journal_id fail-closed",
            proven_by: &[
                "klarbog-plugin-invoice late_compensation_tests::posts_compensation_once_and_rejects_double_post",
                "klarbog-api ui::tests::invoice_flow::ui_compensation_register_and_post",
                "klarbog-mcp invoice_settlement::tests::mcp_claim_and_post_compensation_preview",
            ],
            gaps: &[
                "invoice_compensation_postings append-only link-tabel",
                "accountRoleCompatibility / resolveClaimIncomeAccount",
            ],
        },
        RegisteredRule {
            rule_id: "DK-EMAIL-DELIVERY-001",
            name: "Invoice email delivery must be deterministic, idempotent, and append-only logged",
            source_id: "DK-BOGFORINGSLOVEN-2022-700",
            provisions: &["§ 7, stk. 1", "§ 9, stk. 1"],
            severity: "hard_stop",
            enforced_by: "klarbog-mail SMTP; invoice-plugin send_invoice_email attaches issued PDF; dual-write email_send_log.jsonl + SQLite email_send_log (message_id UNIQUE) + audit_log invoice_email_send; UI send_email + send_reminder",
            proven_by: &[
                "klarbog-mail tests",
                "klarbog-plugin-invoice email::tests",
                "klarbog-plugin-invoice email_ledger::tests",
                "klarbog-store-sqlite email_audit::tests",
                "klarbog-invoice-pdf tests",
                "klarbog-api ui::tests::invoice_flow::ui_send_reminder_compound",
                "klarbog-mcp email::tests",
            ],
            gaps: &[],
        },
        RegisteredRule {
            rule_id: "DK-INVOICE-ISSUE-001",
            name: "Issued invoices must be stored immutably with sequential invoice numbers",
            source_id: "DK-MOMSBEKENDTGORELSEN-2023-1435",
            provisions: &["§ 58, stk. 1, nr. 2"],
            severity: "hard_stop",
            enforced_by: "invoice-plugin: fortløbende {scope}-{NNNN}; immutable JSON+PDF under invoices/issued/ with top-level currency; sha256 + pdf_sha256 in documents.json notes; retain_until",
            proven_by: &[
                "klarbog-plugin-invoice invoice_numbers::tests",
                "klarbog-plugin-documents issued_invoice::tests::attach_writes_immutable_json",
                "klarbog-plugin-documents issued_invoice::tests::attach_writes_pdf_alongside_json",
                "klarbog-invoice-pdf tests::deterministic_same_payload_same_hash",
            ],
            gaps: &[
                "FX conversion (fxRateToDkk / DKK totals) not in Rust product yet",
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn registry_entries_are_complete_and_unique() {
        let rules = registered_rules();
        assert!(!rules.is_empty());
        let mut seen = HashSet::new();
        for rule in rules {
            assert!(rule.rule_id.starts_with("DK-"), "{}", rule.rule_id);
            assert!(seen.insert(rule.rule_id), "duplicate {}", rule.rule_id);
            assert!(!rule.name.is_empty());
            assert!(rule.source_id.starts_with("DK-"));
            assert!(
                !rule.provisions.is_empty(),
                "{}: no provisions",
                rule.rule_id
            );
            assert!(rule.provisions.iter().all(|p| p.starts_with("§")));
            assert_eq!(rule.severity, "hard_stop");
            assert!(!rule.enforced_by.is_empty());
            assert!(!rule.proven_by.is_empty(), "{}: no proof", rule.rule_id);
        }
    }
}
