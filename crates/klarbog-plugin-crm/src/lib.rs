//! CRM plugin — parties in `parties.json`. No journal-write capability (ADR-004).

mod store;

pub use store::{get_party, list_parties, upsert_party, CrmError, PARTIES_FILENAME};

use klarbog_plugin::{Capability, Plugin};
use klarbog_types::PartyId;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Party {
    pub id: PartyId,
    pub display_name: String,
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
    ) -> Result<Party, CrmError> {
        upsert_party(company, id, display_name.into())
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
        let party = crm.upsert(&co, "Nordic Supply", None).unwrap();
        assert!(crm.get(&co, &party.id).unwrap().is_some());
        assert_eq!(crm.list(&co).unwrap().len(), 1);
        assert!(co.join(PARTIES_FILENAME).exists());
    }
}
