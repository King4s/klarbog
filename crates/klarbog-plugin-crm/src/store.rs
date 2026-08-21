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

pub fn upsert_party(
    company: &Path,
    id: Option<PartyId>,
    display_name: String,
    kind: PartyKind,
) -> Result<Party, CrmError> {
    if display_name.trim().is_empty() {
        return Err(CrmError::EmptyName);
    }
    let party_id = id.unwrap_or_else(|| slug_id(&display_name));
    let party = Party {
        id: party_id.clone(),
        display_name,
        kind,
    };
    let mut file = load(company)?;
    if let Some(existing) = file.parties.iter_mut().find(|p| p.id == party_id) {
        *existing = party.clone();
    } else {
        file.parties.push(party.clone());
    }
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
        let first = upsert_party(&co, None, "Acme ApS".into(), PartyKind::Business).unwrap();
        let again = get_party(&co, &first.id).unwrap().unwrap();
        assert_eq!(again.display_name, "Acme ApS");
        upsert_party(
            &co,
            Some(first.id.clone()),
            "Acme A/S".into(),
            PartyKind::Business,
        )
        .unwrap();
        let listed = list_parties(&co).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].display_name, "Acme A/S");
    }
}
