//! CRM plugin — parties in `parties.json`. No journal-write capability (ADR-004).

mod payment_terms;
mod store;

pub use payment_terms::{payment_terms_deviation_note, resolve_payment_terms_days};
pub use store::{get_party, list_parties, upsert_party, CrmError, PARTIES_FILENAME};

use klarbog_plugin::{Capability, Plugin};
use klarbog_types::PartyId;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Beløbskonvention pr. partstype (ADR-020): privat faktureres inkl. moms,
/// erhverv ekskl. moms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartyKind {
    #[default]
    Private,
    Business,
}

impl PartyKind {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "private" | "privat" => Some(Self::Private),
            "business" | "erhverv" => Some(Self::Business),
            _ => None,
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Business => "business",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Party {
    pub id: PartyId,
    pub display_name: String,
    /// Legacy parties.json without the field deserializes as `private`.
    #[serde(default)]
    pub kind: PartyKind,
    /// `None` = inherit company profile payment terms at invoice issue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_terms_days: Option<u32>,
}

pub struct CrmPlugin;

impl Default for CrmPlugin {
    fn default() -> Self {
        Self
    }
}

impl CrmPlugin {
    pub fn upsert(
        &self,
        company: &Path,
        display_name: impl Into<String>,
        id: Option<PartyId>,
        kind: PartyKind,
    ) -> Result<Party, CrmError> {
        upsert_party(company, id, display_name.into(), kind, None)
    }

    pub fn get(&self, company: &Path, id: &PartyId) -> Result<Option<Party>, CrmError> {
        get_party(company, id)
    }

    pub fn list(&self, company: &Path) -> Result<Vec<Party>, CrmError> {
        list_parties(company)
    }
}

impl Plugin for CrmPlugin {
    fn id(&self) -> &'static str {
        "crm"
    }
    fn version(&self) -> &'static str {
        "0.1.0"
    }
    fn capabilities(&self) -> &'static [Capability] {
        &[Capability::Read, Capability::CrmWrite]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn crm_has_no_journal_write() {
        let p = CrmPlugin;
        assert!(!p.has_journal_write());
    }

    #[test]
    fn upsert_list_get_roundtrip() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        fs::create_dir_all(&co).unwrap();
        let crm = CrmPlugin;
        let party = crm
            .upsert(&co, "Nordic Supply", None, PartyKind::Business)
            .unwrap();
        let got = crm.get(&co, &party.id).unwrap().unwrap();
        assert_eq!(got.kind, PartyKind::Business);
        assert_eq!(crm.list(&co).unwrap().len(), 1);
        assert!(co.join(PARTIES_FILENAME).exists());
    }

    #[test]
    fn legacy_party_without_kind_is_private() {
        let json = r#"{"parties":[{"id":"party_x","display_name":"X"}]}"#;
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        fs::create_dir_all(&co).unwrap();
        fs::write(co.join(PARTIES_FILENAME), json).unwrap();
        let listed = list_parties(&co).unwrap();
        assert_eq!(listed[0].kind, PartyKind::Private);
    }
}
