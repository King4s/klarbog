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
            enforced_by: "retention-plugin: RetentionPolicy (retain_days) + purge dry-run→confirm",
            proven_by: &["klarbog-plugin-retention retention/backup/purge tests"],
            gaps: &[
                "retain_until pr. dokument/postering/banktransaktion (porten har global retain_days)",
                "retention-status-rapport pr. skæringsdato",
            ],
        },
        RegisteredRule {
            rule_id: "DK-BOOKKEEPING-BANK-IMPORT-001",
            name: "Imported bank transactions must preserve transaction date, amount, text, and traceable source batch",
            source_id: "DK-BOGFORINGSLOVEN-2022-700",
            provisions: &["§ 7, stk. 1", "§ 9, stk. 2"],
            severity: "hard_stop",
            enforced_by: "bank-plugin: import-preview bevarer dato/tekst/beløb pr. række; ingen stiltiende mutation",
            proven_by: &["klarbog-plugin-bank map/import tests"],
            gaps: &["sporbar kilde-batch (source batch id) på importerede rækker"],
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
            enforced_by: "invoice-plugin credit_journal_suggestion: reference via memo invoice:{id}:credit · {reason} (begrundelse påkrævet), eksakt negation inkl. momsben, fail-closed på betalte",
            proven_by: &[
                "klarbog-plugin-invoice draft::tests::credit_note_negates_send_booking_incl_vat",
                "klarbog-plugin-invoice draft::tests::credit_note_requires_reason_as_in_original",
            ],
            gaps: &[
                "CN-nummerserie pr. regnskabsår (sequences)",
                "delkreditering med kumulativt loft mod original-brutto",
                "kreditnota som immutabelt dokument (sha256/retention)",
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
        for id in [
            "DK-CREDIT-NOTE-001",
            "DK-BOOKKEEPING-RETENTION-001",
            "DK-BOOKKEEPING-BANK-IMPORT-001",
        ] {
            let rule = rules.iter().find(|r| r.rule_id == id).expect(id);
            assert!(!rule.gaps.is_empty(), "{id} must declare gaps");
        }
    }
}
