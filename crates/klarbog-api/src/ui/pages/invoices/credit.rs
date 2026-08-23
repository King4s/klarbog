//! Two-phase credit note (ADR-020 / DK-CREDIT-NOTE-001): preview peeks the
//! next CN number and builds a (partial or full) credit entry; commit
//! reserves the number, posts, and records the credit ledger row (void
//! only when cumulative credits reach original gross).

use std::path::Path;

use axum::response::Response;
use chrono::Utc;
use klarbog_core::journal_commit;
use klarbog_journal::JournalEntry;
use klarbog_plugin_documents::attach_credit_note;
use klarbog_plugin_invoice::{
    credit_amounts_from_entry, credit_journal_suggestion, credit_note_no_from_memo,
    credit_reason_from_memo, get_invoice, record_credit_note, reserve_credit_note_number,
    resolve_credit_note_number, Invoice, InvoiceConfig, InvoiceId, InvoiceStatus,
};
use klarbog_types::Actor;

use super::super::common::html_ok;
use super::form::InvoiceActionForm;
use super::post::preview_with_pending;
use super::view::load_page;
use crate::AppState;

fn creditable(path: &Path, id: &InvoiceId) -> Result<Invoice, String> {
    let invoice = match get_invoice(path, id) {
        Ok(Some(inv)) => inv,
        Ok(None) => return Err(format!("Ukendt: {id}")),
        Err(e) => return Err(e.to_string()),
    };
    if !invoice.status.allows_credit() {
        return Err(format!(
            "{id} kan ikke krediteres (status {:?}, kræver sent)",
            invoice.status
        ));
    }
    if !invoice.payments.is_empty() {
        return Err(format!(
            "{id} har registrerede betalinger — kreditnota afvist"
        ));
    }
    match invoice.creditable_remaining_minor() {
        Ok(r) if r > 0 => Ok(invoice),
        Ok(_) => Err(format!("{id} er allerede fuldt krediteret")),
        Err(e) => Err(e.to_string()),
    }
}

fn parse_credit_amount(raw: &Option<String>) -> Result<Option<i64>, String> {
    let Some(s) = raw.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    s.parse::<i64>()
        .map(Some)
        .map_err(|_| format!("Kreditbeløb skal være heltal (øre), fik {s}"))
}

pub(super) async fn credit_preview(
    state: &AppState,
    company: &str,
    path: &Path,
    actor: &Actor,
    form: &InvoiceActionForm,
) -> Response {
    let id = InvoiceId::new(form.invoice_id.clone().unwrap_or_default());
    let reason = form.credit_reason.clone().unwrap_or_default();
    let invoice = match creditable(path, &id) {
        Ok(inv) => inv,
        Err(e) => return html_ok(load_page(state, company, String::new(), e).await),
    };
    let amount = match parse_credit_amount(&form.credit_amount_minor) {
        Ok(a) => a,
        Err(e) => return html_ok(load_page(state, company, String::new(), e).await),
    };
    let cn_no =
        match resolve_credit_note_number(path, Utc::now(), form.credit_note_number.as_deref()) {
            Ok(no) => no,
            Err(e) => {
                return html_ok(load_page(state, company, String::new(), e.to_string()).await)
            }
        };
    match credit_journal_suggestion(
        &invoice,
        &cn_no,
        &reason,
        amount,
        actor,
        &InvoiceConfig::default(),
    ) {
        Ok(entry) => {
            let label = if amount.is_some() {
                "Delkreditnota"
            } else {
                "Kreditnota"
            };
            preview_with_pending(state, company, actor, &id, entry, label, "commit_credit").await
        }
        Err(e) => html_ok(load_page(state, company, String::new(), e.to_string()).await),
    }
}

pub(super) async fn commit_credit(
    state: &AppState,
    company: &str,
    path: &Path,
    actor: &Actor,
    form: InvoiceActionForm,
) -> Response {
    let id = InvoiceId::new(form.invoice_id.unwrap_or_default());
    if let Err(e) = creditable(path, &id) {
        return html_ok(load_page(state, company, String::new(), e).await);
    }
    let token = form.confirm_token.unwrap_or_default();
    let entry: JournalEntry = match serde_json::from_str(form.entry_json.unwrap_or_default().trim())
    {
        Ok(e) => e,
        Err(e) => {
            return html_ok(
                load_page(
                    state,
                    company,
                    String::new(),
                    format!("Ugyldig entry_json: {e}"),
                )
                .await,
            );
        }
    };
    let Some(cn_no) = credit_note_no_from_memo(&entry.memo) else {
        return html_ok(
            load_page(
                state,
                company,
                String::new(),
                "Kreditnota-memo mangler CN-nummer — kør preview igen".into(),
            )
            .await,
        );
    };
    let cfg = InvoiceConfig::default();
    let (net, vat, gross) = match credit_amounts_from_entry(&entry, &cfg) {
        Ok(a) => a,
        Err(e) => {
            return html_ok(load_page(state, company, String::new(), e.to_string()).await);
        }
    };
    if let Err(e) = reserve_credit_note_number(path, entry.as_of, &cn_no) {
        return html_ok(
            load_page(
                state,
                company,
                String::new(),
                format!("CN-nummer kunne ikke reserveres ({cn_no}): {e} — kør preview igen"),
            )
            .await,
        );
    }
    match journal_commit(
        &state.allowlist_root,
        Path::new(company),
        entry.clone(),
        actor,
        token.trim(),
        &state.confirm,
        &state.registry,
    )
    .await
    {
        Ok(r) => {
            let invoice = match get_invoice(path, &id) {
                Ok(Some(inv)) => inv,
                Ok(None) => {
                    return html_ok(
                        load_page(
                            state,
                            company,
                            String::new(),
                            format!("Kreditnota bogført ({}) men faktura mangler", r.posted.id),
                        )
                        .await,
                    );
                }
                Err(e) => {
                    return html_ok(
                        load_page(
                            state,
                            company,
                            String::new(),
                            format!(
                                "Kreditnota bogført ({}) men faktura kunne ikke læses: {e}",
                                r.posted.id
                            ),
                        )
                        .await,
                    );
                }
            };
            let credited_so_far = match invoice.credited_gross_minor() {
                Ok(v) => v,
                Err(e) => {
                    return html_ok(load_page(state, company, String::new(), e.to_string()).await);
                }
            };
            let remaining_before = match invoice.creditable_remaining_minor() {
                Ok(v) => v,
                Err(e) => {
                    return html_ok(load_page(state, company, String::new(), e.to_string()).await);
                }
            };
            let issue_date = entry.as_of.format("%Y-%m-%d").to_string();
            let reason = credit_reason_from_memo(&entry.memo).unwrap_or_default();
            let doc = match attach_credit_note(
                path,
                &cn_no,
                &id,
                &invoice.party_id,
                &issue_date,
                &reason,
                net,
                vat,
                gross,
                credited_so_far,
                remaining_before - gross,
                entry.as_of,
            )
            .await
            {
                Ok(d) => d,
                Err(e) => {
                    return html_ok(
                        load_page(
                            state,
                            company,
                            String::new(),
                            format!(
                                "Kreditnota bogført ({}) men dokument fejlede: {e}",
                                r.posted.id
                            ),
                        )
                        .await,
                    );
                }
            };
            match record_credit_note(
                path,
                &id,
                &cn_no,
                net,
                vat,
                gross,
                Some(doc.id.to_string()),
                doc.sha256.clone(),
            ) {
                Ok(inv) => {
                    let status = match inv.status {
                        InvoiceStatus::Void => "void (fuldt krediteret)".to_string(),
                        other => format!("{other:?} (delkredit)"),
                    };
                    html_ok(
                        load_page(
                            state,
                            company,
                            format!(
                                "Kreditnota {cn_no} bogført · posted {} · {id} · {status}",
                                r.posted.id
                            ),
                            String::new(),
                        )
                        .await,
                    )
                }
                Err(e) => html_ok(
                    load_page(
                        state,
                        company,
                        String::new(),
                        format!(
                            "Kreditnota bogført ({}) men statusskift fejlede: {e}",
                            r.posted.id
                        ),
                    )
                    .await,
                ),
            }
        }
        Err(e) => html_ok(load_page(state, company, String::new(), e.to_string()).await),
    }
}
