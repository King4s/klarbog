//! Fortløbende nummerserier — porteret fra originalens `core/sequences.ts`
//! og `credit-notes.ts` (DK-CREDIT-NOTE-001 / DK-INVOICE-ISSUE-001-mønstret):
//! `CN-{regnskabsår}-{NNNN}` med floor fra allerede udstedte numre og
//! fail-closed reservation ved kapløb (originalens reserveSequenceValue).
//!
//! Portens to-faser: preview KIGGER på næste nummer (uden at skrive),
//! nummeret bages ind i det digest-bundne entry-memo, og commit RESERVERER
//! præcis det nummer — afvist hvis en anden kreditnota kom først.
//! Regnskabsår læses fra `policy.json` (originalens company.fiscalYear*).

use crate::{Invoice, InvoiceError};
use chrono::{DateTime, Utc};
use klarbog_types::{fiscal_year_identifier_label, load_fiscal_settings};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub const SEQUENCES_FILENAME: &str = "sequences.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct SequencesFile {
    /// `kind:scope` → senest udstedte værdi.
    values: BTreeMap<String, u32>,
}

pub(crate) fn load(company: &Path) -> Result<SequencesFile, InvoiceError> {
    let path = company.join(SEQUENCES_FILENAME);
    if !path.exists() {
        return Ok(SequencesFile::default());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

pub(crate) fn save(company: &Path, file: &SequencesFile) -> Result<(), InvoiceError> {
    let json = serde_json::to_string_pretty(file)?;
    fs::write(company.join(SEQUENCES_FILENAME), json)?;
    Ok(())
}

fn key(kind: &str, scope: &str) -> String {
    format!("{kind}:{scope}")
}

/// Næste værdi uden at skrive (max af gemt værdi og floor, plus 1).
pub(crate) fn peek_value(
    company: &Path,
    kind: &str,
    scope: &str,
    floor: u32,
) -> Result<u32, InvoiceError> {
    let file = load(company)?;
    let current = file.values.get(&key(kind, scope)).copied().unwrap_or(0);
    Ok(current.max(floor) + 1)
}

/// Reservér præcis `requested` — fail-closed hvis det ikke længere er næste
/// fortløbende værdi (originalens reserveSequenceValue-semantik).
pub(crate) fn reserve_value(
    company: &Path,
    kind: &str,
    scope: &str,
    requested: u32,
    floor: u32,
) -> Result<(), InvoiceError> {
    let mut file = load(company)?;
    let current = file.values.get(&key(kind, scope)).copied().unwrap_or(0);
    let expected = current.max(floor) + 1;
    if requested != expected {
        return Err(InvoiceError::SequenceConflict {
            requested,
            expected,
        });
    }
    file.values.insert(key(kind, scope), requested);
    save(company, &file)
}

/// Regnskabsårs-label for en dato (originalens fiscalYearLabelFromDate).
fn fiscal_scope(company: &Path, as_of: DateTime<Utc>) -> String {
    let settings = load_fiscal_settings(company);
    fiscal_year_identifier_label(as_of.date_naive(), settings)
}

fn format_cn(scope: &str, value: u32) -> String {
    format!("CN-{scope}-{value:04}")
}

/// Generic canonical `CN-YYYY-NNNN` (originalens validateManualCreditNoteNumberScope).
fn parse_generic_canonical(credit_note_number: &str) -> Option<(&str, &str)> {
    let rest = credit_note_number.strip_prefix("CN-")?;
    let (year, seq) = rest.split_once('-')?;
    if year.len() != 4 || !year.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if seq.len() != 4 || !seq.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((year, seq))
}

/// Fail-closed hvis et manuelt nummer er kanonisk men fra forkert regnskabsår.
pub fn validate_manual_credit_note_number_scope(
    company: &Path,
    as_of: DateTime<Utc>,
    credit_note_number: &str,
) -> Result<(), InvoiceError> {
    if let Some((year, _)) = parse_generic_canonical(credit_note_number.trim()) {
        let scope = fiscal_scope(company, as_of);
        if year != scope {
            return Err(InvoiceError::ManualCreditNoteScopeMismatch {
                number: credit_note_number.trim().to_string(),
                scope,
            });
        }
    }
    Ok(())
}

/// Preview: auto-peek eller manuelt nummer (med scope-validering).
pub fn resolve_credit_note_number(
    company: &Path,
    as_of: DateTime<Utc>,
    manual: Option<&str>,
) -> Result<String, InvoiceError> {
    if let Some(raw) = manual.map(str::trim).filter(|s| !s.is_empty()) {
        validate_manual_credit_note_number_scope(company, as_of, raw)?;
        Ok(raw.to_string())
    } else {
        peek_credit_note_number(company, as_of)
    }
}

/// Floor fra allerede udstedte CN-numre i samme scope (originalens
/// creditNoteSequenceState: MAX over eksisterende dokumenter).
fn credit_note_floor(invoices: &[Invoice], scope: &str) -> u32 {
    let prefix = format!("CN-{scope}-");
    let mut max = 0u32;
    for inv in invoices {
        if let Some(no) = inv.credit_note_no.as_deref() {
            if let Some(n) = no.strip_prefix(&prefix).and_then(|s| s.parse().ok()) {
                max = max.max(n);
            }
        }
        for c in &inv.credits {
            if let Some(n) = c
                .credit_note_no
                .strip_prefix(&prefix)
                .and_then(|s| s.parse().ok())
            {
                max = max.max(n);
            }
        }
    }
    max
}

/// Preview: næste CN-nummer for datoen, uden at reservere.
pub fn peek_credit_note_number(
    company: &Path,
    as_of: DateTime<Utc>,
) -> Result<String, InvoiceError> {
    let scope = fiscal_scope(company, as_of);
    let invoices = crate::store::list_invoices(company)?;
    let floor = credit_note_floor(&invoices, &scope);
    let value = peek_value(company, "credit_note", &scope, floor)?;
    Ok(format_cn(&scope, value))
}

/// Commit: reservér præcis `number` når det matcher `CN-{scope}-NNNN`.
/// Ikke-kanoniske manuelle numre springer sekvensen over (originalens
/// reserveManualCreditNoteNumber).
pub fn reserve_credit_note_number(
    company: &Path,
    as_of: DateTime<Utc>,
    number: &str,
) -> Result<(), InvoiceError> {
    let scope = fiscal_scope(company, as_of);
    let prefix = format!("CN-{scope}-");
    let Some(suffix) = number
        .strip_prefix(&prefix)
        .filter(|n| n.len() == 4 && n.chars().all(|c| c.is_ascii_digit()))
    else {
        return Ok(());
    };
    let value: u32 = suffix
        .parse()
        .map_err(|_| InvoiceError::BadCreditNoteNumber(number.to_string()))?;
    let invoices = crate::store::list_invoices(company)?;
    let floor = credit_note_floor(&invoices, &scope);
    reserve_value(company, "credit_note", &scope, value, floor)
}

/// Udtræk CN-nummeret fra et credit-memo `invoice:{id}:credit:{CN} · {reason}`.
pub fn credit_note_no_from_memo(memo: &str) -> Option<String> {
    let tail = memo.split(":credit:").nth(1)?;
    let no = tail.split(" · ").next()?.trim();
    if no.starts_with("CN-") {
        Some(no.to_string())
    } else {
        None
    }
}

/// Udtræk begrundelsen fra et credit-memo `invoice:{id}:credit:{CN} · {reason}`.
pub fn credit_reason_from_memo(memo: &str) -> Option<String> {
    let tail = memo.split(":credit:").nth(1)?;
    let reason = tail.split(" · ").nth(1)?.trim();
    if reason.is_empty() {
        None
    } else {
        Some(reason.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn dt(s: &str) -> DateTime<Utc> {
        format!("{s}T12:00:00Z").parse().unwrap()
    }

    #[test]
    fn peek_does_not_consume_and_reserve_advances() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        std::fs::create_dir_all(&co).unwrap();
        let as_of = dt("2026-05-20");
        assert_eq!(peek_credit_note_number(&co, as_of).unwrap(), "CN-2026-0001");
        assert_eq!(peek_credit_note_number(&co, as_of).unwrap(), "CN-2026-0001");
        reserve_credit_note_number(&co, as_of, "CN-2026-0001").unwrap();
        assert_eq!(peek_credit_note_number(&co, as_of).unwrap(), "CN-2026-0002");
    }

    #[test]
    fn reserve_is_fail_closed_on_race() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        std::fs::create_dir_all(&co).unwrap();
        let as_of = dt("2026-05-20");
        reserve_credit_note_number(&co, as_of, "CN-2026-0001").unwrap();
        // En anden kreditnota tog 0001 — et gammelt preview må ikke committes.
        let err = reserve_credit_note_number(&co, as_of, "CN-2026-0001").unwrap_err();
        assert!(matches!(
            err,
            InvoiceError::SequenceConflict {
                requested: 1,
                expected: 2
            }
        ));
        // Og man kan ikke springe frem.
        assert!(reserve_credit_note_number(&co, as_of, "CN-2026-0005").is_err());
    }

    #[test]
    fn scope_resets_per_fiscal_year() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        std::fs::create_dir_all(&co).unwrap();
        reserve_credit_note_number(&co, dt("2026-12-31"), "CN-2026-0001").unwrap();
        assert_eq!(
            peek_credit_note_number(&co, dt("2027-01-01")).unwrap(),
            "CN-2027-0001"
        );
    }

    #[test]
    fn non_canonical_manual_skips_sequence_reservation() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        std::fs::create_dir_all(&co).unwrap();
        let as_of = dt("2026-05-20");
        reserve_credit_note_number(&co, as_of, "KREDIT-42").unwrap();
        assert_eq!(peek_credit_note_number(&co, as_of).unwrap(), "CN-2026-0001");
    }

    #[test]
    fn manual_scope_mismatch_is_rejected() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        std::fs::create_dir_all(&co).unwrap();
        let as_of = dt("2026-05-20");
        let err = validate_manual_credit_note_number_scope(&co, as_of, "CN-2099-0001").unwrap_err();
        assert!(matches!(
            err,
            InvoiceError::ManualCreditNoteScopeMismatch { .. }
        ));
    }

    #[test]
    fn resolve_uses_manual_when_provided() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        std::fs::create_dir_all(&co).unwrap();
        let as_of = dt("2026-05-20");
        assert_eq!(
            resolve_credit_note_number(&co, as_of, Some("KREDIT-7")).unwrap(),
            "KREDIT-7"
        );
        assert_eq!(
            resolve_credit_note_number(&co, as_of, None).unwrap(),
            "CN-2026-0001"
        );
    }

    #[test]
    fn manual_canonical_ahead_of_sequence_fails() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        std::fs::create_dir_all(&co).unwrap();
        let as_of = dt("2026-05-20");
        assert!(matches!(
            reserve_credit_note_number(&co, as_of, "CN-2026-0005"),
            Err(InvoiceError::SequenceConflict { .. })
        ));
    }

    #[test]
    fn july_fiscal_scope_on_cn_numbers() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        std::fs::create_dir_all(&co).unwrap();
        std::fs::write(
            co.join("policy.json"),
            r#"{"name":"T","actors":["u"],"fiscalYearStartMonth":7,"fiscalYearLabelStrategy":"end-year"}"#,
        )
        .unwrap();
        assert_eq!(
            peek_credit_note_number(&co, dt("2026-12-31")).unwrap(),
            "CN-2027-0001"
        );
    }

    #[test]
    fn memo_roundtrip() {
        assert_eq!(
            credit_note_no_from_memo("invoice:inv_1:credit:CN-2026-0007 · forkert beløb")
                .as_deref(),
            Some("CN-2026-0007")
        );
        assert_eq!(credit_note_no_from_memo("invoice:inv_1:credit · x"), None);
        assert_eq!(
            credit_reason_from_memo("invoice:inv_1:credit:CN-2026-0007 · forkert beløb").as_deref(),
            Some("forkert beløb")
        );
    }
}
