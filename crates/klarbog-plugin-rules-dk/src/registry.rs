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
            enforced_by: "retention-plugin: retain_until på dokument/journal/bank ved oprettelse + retention-status-rapport (CLI: retention-status); global retain_days + purge dry-run→confirm",
            proven_by: &[
                "klarbog-types retention_deadline::tests",
                "klarbog-plugin-retention status_report::tests::report_counts_objects_and_expiry_after_deadline",
                "klarbog-plugin-bank store::tests::commit_assigns_batch_hash_and_skips_duplicates",
            ],
            gaps: &[
                "konfigurerbart regnskabsår (porten bruger kalenderår Jan–Dec; originalen læser company.fiscalYearStartMonth)",
            ],
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
            enforced_by: "invoice-plugin: fortløbende CN-nr pr. regnskabsår; del- og fuld kreditnota med kumulativt loft mod original-brutto (proportional moms, residual på sidste); memo invoice:{id}:credit:{CN} · {reason}; fail-closed på betalte",
            proven_by: &[
                "klarbog-plugin-invoice credit::tests::full_credit_negates_send_booking_incl_vat",
                "klarbog-plugin-invoice credit::tests::partial_then_residual_lands_exactly_on_original",
                "klarbog-plugin-invoice credit::tests::cumulative_cap_rejects_over_credit",
                "klarbog-plugin-invoice sequences::tests::reserve_is_fail_closed_on_race",
            ],
            gaps: &[
                "kreditnota som immutabelt dokument (sha256/retention)",
                "konfigurerbart regnskabsår (porten bruger kalenderår som originalens default)",
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

    #[test]
    fn partially_ported_rules_declare_their_gaps() {
        // Honest coverage: rules whose original machine_rule the port only
        // partially satisfies must say so — the original's coverage report
        // warns against self-attestation.
        let rules = registered_rules();
        for id in ["DK-CREDIT-NOTE-001", "DK-BOOKKEEPING-RETENTION-001"] {
            let rule = rules.iter().find(|r| r.rule_id == id).expect(id);
            assert!(!rule.gaps.is_empty(), "{id} must declare gaps");
        }
    }
}
