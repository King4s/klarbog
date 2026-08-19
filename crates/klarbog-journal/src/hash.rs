use klarbog_types::Actor;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::entry::JournalEntry;

pub fn entry_digest(id: &Uuid, entry: &JournalEntry, prev: Option<&str>) -> String {
    let mut h = Sha256::new();
    h.update(id.as_bytes());
    h.update(entry.as_of.to_rfc3339().as_bytes());
    h.update(entry.memo.as_bytes());
    h.update(entry.actor.as_tag().as_bytes());
    for leg in &entry.legs {
        h.update(leg.account.as_bytes());
        h.update(format!("{:?}", leg.direction).as_bytes());
        h.update(leg.amount.minor().to_le_bytes());
        h.update(leg.currency.as_str().as_bytes());
    }
    if let Some(p) = prev {
        h.update(p.as_bytes());
    }
    // silence unused import warning pattern for Actor in signature docs
    let _: &Actor = &entry.actor;
    hex::encode(h.finalize())
}
