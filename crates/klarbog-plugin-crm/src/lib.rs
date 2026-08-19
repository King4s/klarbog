//! CRM plugin — parties/pipeline stubs. No journal-write capability (ADR-004).

use klarbog_plugin::{Capability, Plugin};
use klarbog_types::PartyId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Party {
    pub id: PartyId,
    pub display_name: String,
}

pub struct CrmPlugin {
    parties: Mutex<HashMap<String, Party>>,
}

impl Default for CrmPlugin {
    fn default() -> Self {
        Self {
            parties: Mutex::new(HashMap::new()),
        }
    }
}

impl CrmPlugin {
    pub fn upsert(&self, display_name: impl Into<String>) -> Party {
        let name = display_name.into();
        let id = PartyId::new(format!("party_{}", name.to_lowercase().replace(' ', "_")));
        let party = Party {
            id: id.clone(),
            display_name: name,
        };
        self.parties
            .lock()
            .expect("crm lock")
            .insert(id.as_str().to_string(), party.clone());
        party
    }

    pub fn get(&self, id: &PartyId) -> Option<Party> {
        self.parties
            .lock()
            .expect("crm lock")
            .get(id.as_str())
            .cloned()
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
        // No JournalWrite — compile-time + runtime signal
        &[Capability::Read, Capability::CrmWrite]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crm_has_no_journal_write() {
        let p = CrmPlugin::default();
        assert!(!p.has_journal_write());
        let party = p.upsert("Acme ApS");
        assert!(p.get(&party.id).is_some());
    }
}
