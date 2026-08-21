//! Bank reconcile commit: post the digest-bound payment entry, then record
//! the payment on the invoice so it can no longer be collected or credited
//! (ADR-020). Returns ok/err flash text.

use std::path::Path;

use klarbog_core::journal_commit;
use klarbog_plugin_invoice::{record_payment, InvoiceId};
use klarbog_types::Actor;

use crate::AppState;

pub(super) async fn commit_apply(
    state: &AppState,
    company: &str,
    path: &Path,
    actor: &Actor,
    invoice_id: &str,
    entry_json: &str,
    confirm_token: &str,
) -> Result<String, String> {
    let entry: klarbog_journal::JournalEntry =
        serde_json::from_str(entry_json.trim()).map_err(|e| format!("Ugyldig entry_json: {e}"))?;
    // The payment amount is the (digest-bound) entry's first leg; both legs
    // of a payment suggestion carry the same amount.
    let payment_minor = entry.legs.first().map(|l| l.amount.minor()).unwrap_or(0);
    let invoice_id = invoice_id.trim();
    let r = journal_commit(
        &state.allowlist_root,
        Path::new(company),
        entry,
        actor,
        confirm_token.trim(),
        &state.confirm,
        &state.registry,
    )
    .await
    .map_err(|e| e.to_string())?;
    if invoice_id.is_empty() {
        return Err(format!(
            "Afstemning bogført ({}) men invoice_id mangler — betaling ikke registreret på faktura",
            r.posted.id
        ));
    }
    match record_payment(path, &InvoiceId::new(invoice_id.to_string()), payment_minor) {
        Ok(inv) => Ok(format!(
            "Afstemning bogført · posted {} · {} sat til {:?}",
            r.posted.id, invoice_id, inv.status
        )),
        Err(e) => Err(format!(
            "Afstemning bogført ({}) men betaling ikke registreret på {invoice_id}: {e}",
            r.posted.id
        )),
    }
}
