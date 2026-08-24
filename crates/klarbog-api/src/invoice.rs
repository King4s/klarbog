//! Invoice draft HTTP handlers (slice 6). AuthZ mirrors CRM routes.

use crate::actor::parse_actor;
use crate::AppState;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use klarbog_core::{assert_company_path, open_existing, CoreError};
use klarbog_plugin_invoice::{
    create_draft_from_new, get_invoice, journal_suggestion, list_invoices, InvoiceConfig,
    InvoiceError, InvoiceId, InvoiceKind, NewLine,
};
use klarbog_types::{Actor, Envelope, PartyId};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
pub struct CreateBody {
    pub company: String,
    pub party_id: String,
    pub kind: InvoiceKind,
    pub lines: Vec<NewLine>,
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub company: String,
    pub invoice_id: Option<String>,
}

pub(crate) async fn authorize_company(
    allowlist_root: &Path,
    company: &Path,
    actor: &Actor,
) -> Result<PathBuf, CoreError> {
    let path = assert_company_path(allowlist_root, company)?;
    open_existing(&path).await?.authorize(actor)?;
    Ok(path)
}

pub(crate) fn map_core(err: CoreError) -> (StatusCode, Envelope<Value>) {
    match err {
        CoreError::ActorDenied(tag) => (
            StatusCode::FORBIDDEN,
            Envelope::err([format!("actor not in policy: {tag}")]),
        ),
        CoreError::Path(e) => (StatusCode::BAD_REQUEST, Envelope::err([e.to_string()])),
        CoreError::Store(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([e.to_string()]),
        ),
        CoreError::Other(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([e.to_string()]),
        ),
        other => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([other.to_string()]),
        ),
    }
}

pub(crate) fn map_invoice(err: InvoiceError) -> (StatusCode, Envelope<Value>) {
    match err {
        InvoiceError::NotFound(id) => (
            StatusCode::NOT_FOUND,
            Envelope::err([format!("invoice not found: {id}")]),
        ),
        InvoiceError::PartyNotFound(id) => (
            StatusCode::NOT_FOUND,
            Envelope::err([format!("party not found: {id}")]),
        ),
        InvoiceError::NoLines
        | InvoiceError::EmptyDescription
        | InvoiceError::MissingCreditReason
        | InvoiceError::NothingCreditable
        | InvoiceError::CreditAmountInvalid { .. }
        | InvoiceError::CreditExceedsRemaining { .. }
        | InvoiceError::SequenceConflict { .. }
        | InvoiceError::BadCreditNoteNumber(_)
        | InvoiceError::ManualCreditNoteScopeMismatch { .. }
        | InvoiceError::BadInvoiceNumber(_)
        | InvoiceError::ManualInvoiceScopeMismatch { .. }
        | InvoiceError::DueBeforeIssue { .. }
        | InvoiceError::InvalidDueDate(_)
        | InvoiceError::NoStatutoryReferenceRate(_)
        | InvoiceError::InvalidReferenceRate
        | InvoiceError::NoInterestToRegister
        | InvoiceError::DuplicateInterestClaim { .. }
        | InvoiceError::InterestClaimNotFound(_)
        | InvoiceError::InterestClaimAlreadyPosted
        | InvoiceError::NonDkkInvoice(_)
        | InvoiceError::InvalidReminderFee
        | InvoiceError::ReminderFeeExceedsStatutoryMax { .. }
        | InvoiceError::NotOverdueForReminder
        | InvoiceError::MaxRemindersReached { .. }
        | InvoiceError::ReminderTooSoon { .. }
        | InvoiceError::ReminderNotFound(_)
        | InvoiceError::ReminderAlreadyPosted
        | InvoiceError::NonPositiveAmount
        | InvoiceError::InvalidPartialAmount { .. }
        | InvoiceError::Overpay { .. }
        | InvoiceError::NothingRemaining
        | InvoiceError::MixedCurrency => {
            (StatusCode::BAD_REQUEST, Envelope::err([err.to_string()]))
        }
        InvoiceError::Io(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([e.to_string()]),
        ),
        InvoiceError::Json(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([e.to_string()]),
        ),
        InvoiceError::Overflow
        | InvoiceError::Money(_)
        | InvoiceError::Journal(_)
        | InvoiceError::Vat(_) => (StatusCode::BAD_REQUEST, Envelope::err([err.to_string()])),
        InvoiceError::InvalidTransition { .. } | InvoiceError::CreditWithPayments => {
            (StatusCode::CONFLICT, Envelope::err([err.to_string()]))
        }
        InvoiceError::NotSentForEmail
        | InvoiceError::MissingIssuedDocument
        | InvoiceError::MissingRecipientEmail(_)
        | InvoiceError::InvalidRecipientEmail(_)
        | InvoiceError::EmailSendFailed(_) => {
            (StatusCode::BAD_REQUEST, Envelope::err([err.to_string()]))
        }
        InvoiceError::Crm(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([e.to_string()]),
        ),
    }
}

pub async fn create_draft(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateBody>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(body.company);
    let path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    let party_id = PartyId::new(body.party_id);
    let invoice = create_draft_from_new(&path, party_id, body.kind, body.lines).map_err(|e| {
        let (s, env) = map_invoice(e);
        (s, Json(env))
    })?;
    let journal_entry =
        journal_suggestion(&invoice, &actor, &InvoiceConfig::default()).map_err(|e| {
            let (s, env) = map_invoice(e);
            (s, Json(env))
        })?;
    Ok(Json(Envelope::ok(serde_json::json!({
        "invoice": invoice,
        "journal_entry": journal_entry,
    }))))
}

pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(query.company);
    let path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    if let Some(raw_id) = query.invoice_id {
        let id = InvoiceId::new(raw_id);
        let invoice = get_invoice(&path, &id).map_err(|e| {
            let (s, env) = map_invoice(e);
            (s, Json(env))
        })?;
        let invoice = invoice.ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(Envelope::err([format!("invoice not found: {id}")])),
            )
        })?;
        return Ok(Json(Envelope::ok(serde_json::to_value(invoice).unwrap())));
    }
    let invoices = list_invoices(&path).map_err(|e| {
        let (s, env) = map_invoice(e);
        (s, Json(env))
    })?;
    Ok(Json(Envelope::ok(serde_json::to_value(invoices).unwrap())))
}

#[cfg(test)]
#[path = "invoice_http_tests.rs"]
mod http_tests;
