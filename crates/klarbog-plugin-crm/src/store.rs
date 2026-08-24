//! Per-company `parties.json` persistence (ADR-004: CRM domain only).

use crate::{Party, PartyKind};
use klarbog_types::PartyId;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const PARTIES_FILENAME: &str = "parties.json";

#[derive(Debug, Error)]
pub enum CrmError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("party not found: {0}")]
    NotFound(String),
    #[error("display name must not be empty")]
    EmptyName,
    #[error("payment_terms_days must be between 1 and 365, got {0}")]
    InvalidPaymentTerms(u32),
    #[error("invalid email address: {0}")]
    InvalidEmail(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PartiesFile {
    parties: Vec<Party>,
}

fn parties_path(company: &Path) -> PathBuf {
    company.join(PARTIES_FILENAME)
}

fn load(company: &Path) -> Result<PartiesFile, CrmError> {
    let path = parties_path(company);
    if !path.exists() {
        return Ok(PartiesFile::default());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn save(company: &Path, file: &PartiesFile) -> Result<(), CrmError> {
    let json = serde_json::to_string_pretty(file)?;
    fs::write(parties_path(company), json)?;
    Ok(())
}

fn slug_id(name: &str) -> PartyId {
    let slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    PartyId::new(format!("party_{slug}"))
}

pub fn list_parties(company: &Path) -> Result<Vec<Party>, CrmError> {
    Ok(load(company)?.parties)
}

pub fn get_party(company: &Path, id: &PartyId) -> Result<Option<Party>, CrmError> {
    Ok(load(company)?.parties.into_iter().find(|p| p.id == *id))
}

fn validate_payment_terms_days(days: Option<u32>) -> Result<(), CrmError> {
    if let Some(d) = days {
        if !(1..=365).contains(&d) {
            return Err(CrmError::InvalidPaymentTerms(d));
        }
    }
    Ok(())
}

fn validate_email(email: Option<&str>) -> Result<(), CrmError> {
    if let Some(e) = email {
        let t = e.trim();
        if t.is_empty() {
            return Ok(());
        }
        if !t.contains('@') || t.starts_with('@') || t.ends_with('@') || t.contains(' ') {
            return Err(CrmError::InvalidEmail(e.to_string()));
        }
    }
    Ok(())
}

pub fn upsert_party(
    company: &Path,
    id: Option<PartyId>,
    display_name: String,
    kind: PartyKind,
    payment_terms_days: Option<u32>,
    email: Option<String>,
) -> Result<Party, CrmError> {
    if display_name.trim().is_empty() {
        return Err(CrmError::EmptyName);
    }
    validate_payment_terms_days(payment_terms_days)?;
    let email_provided = email.is_some();
    let normalized_email = email.and_then(|e| {
        let t = e.trim().to_string();
        if t.is_empty() {
            None
        } else {
            Some(t)
        }
    });
    validate_email(normalized_email.as_deref())?;
    let party_id = id.unwrap_or_else(|| slug_id(&display_name));
    let mut file = load(company)?;
    let party = if let Some(existing) = file.parties.iter_mut().find(|p| p.id == party_id) {
        let terms = if payment_terms_days.is_some() {
            payment_terms_days
        } else {
            existing.payment_terms_days
        };
        let mail = if email_provided {
            normalized_email
        } else {
            existing.email.clone()
        };
        *existing = Party {
            id: party_id.clone(),
            display_name,
            kind,
            payment_terms_days: terms,
            email: mail,
        };
        existing.clone()
    } else {
        let party = Party {
            id: party_id.clone(),
            display_name,
            kind,
            payment_terms_days,
            email: normalized_email,
        };
        file.parties.push(party.clone());
        party
    };
    save(company, &file)?;
    Ok(party)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn persists_across_reload() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        fs::create_dir_all(&co).unwrap();
        let first = upsert_party(
            &co,
            None,
            "Acme ApS".into(),
            PartyKind::Business,
            None,
            None,
        )
        .unwrap();
        let again = get_party(&co, &first.id).unwrap().unwrap();
        assert_eq!(again.display_name, "Acme ApS");
        upsert_party(
            &co,
            Some(first.id.clone()),
            "Acme A/S".into(),
            PartyKind::Business,
            None,
            None,
        )
        .unwrap();
        let listed = list_parties(&co).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].display_name, "Acme A/S");
    }

    #[test]
    fn update_without_terms_preserves_existing() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        fs::create_dir_all(&co).unwrap();
        let first = upsert_party(
            &co,
            None,
            "Kunde".into(),
            PartyKind::Private,
            Some(14),
            None,
        )
        .unwrap();
        upsert_party(
            &co,
            Some(first.id.clone()),
            "Kunde ApS".into(),
            PartyKind::Private,
            None,
            None,
        )
        .unwrap();
        let got = get_party(&co, &first.id).unwrap().unwrap();
        assert_eq!(got.payment_terms_days, Some(14));
    }

    #[test]
    fn invalid_payment_terms_rejected() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        fs::create_dir_all(&co).unwrap();
        let err =
            upsert_party(&co, None, "Kunde".into(), PartyKind::Private, Some(0), None).unwrap_err();
        assert!(matches!(err, CrmError::InvalidPaymentTerms(0)));
    }
}
