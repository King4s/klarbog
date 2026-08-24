//! Documents + exceptions HTTP handlers (slice 7). AuthZ mirrors CRM routes.

use crate::actor::parse_actor;
use crate::AppState;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use klarbog_core::{assert_company_path, open_existing, CoreError};
use klarbog_plugin_documents::{
    attach_document, get_document, list_documents, remove_document, DocumentError, DocumentId,
    DocumentKind,
};
use klarbog_plugin_invoice::InvoiceId;
use klarbog_types::{Actor, Envelope, PartyId};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
pub struct AttachBody {
    pub company: String,
    pub kind: DocumentKind,
    pub path_hint: String,
    pub party_id: Option<String>,
    pub invoice_id: Option<String>,
    pub notes: Option<String>,
    pub content_base64: Option<String>,
}

#[derive(Deserialize)]
pub struct DocumentListQuery {
    pub company: String,
    pub document_id: Option<String>,
}

#[derive(Deserialize)]
pub struct DeleteBody {
    pub company: String,
    pub document_id: String,
    #[serde(default = "default_delete_object")]
    pub delete_object: bool,
}

fn default_delete_object() -> bool {
    true
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

pub(crate) fn map_doc(err: DocumentError) -> (StatusCode, Envelope<Value>) {
    match err {
        DocumentError::NotFound(id) => (
            StatusCode::NOT_FOUND,
            Envelope::err([format!("document not found: {id}")]),
        ),
        DocumentError::ExceptionNotFound(id) => (
            StatusCode::NOT_FOUND,
            Envelope::err([format!("exception not found: {id}")]),
        ),
        DocumentError::PartyNotFound(id) => (
            StatusCode::NOT_FOUND,
            Envelope::err([format!("party not found: {id}")]),
        ),
        DocumentError::InvoiceNotFound(id) => (
            StatusCode::NOT_FOUND,
            Envelope::err([format!("invoice not found: {id}")]),
        ),
        DocumentError::EmptyCode
        | DocumentError::EmptyMessage
        | DocumentError::InvalidPathHint(_)
        | DocumentError::InvalidIssueDate(_) => {
            (StatusCode::BAD_REQUEST, Envelope::err([err.to_string()]))
        }
        DocumentError::CreditNoteExists(no) => (
            StatusCode::CONFLICT,
            Envelope::err([format!("credit note already exists: {no}")]),
        ),
        DocumentError::IssuedInvoiceExists(no) => (
            StatusCode::CONFLICT,
            Envelope::err([format!("issued invoice already exists: {no}")]),
        ),
        DocumentError::Io(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([e.to_string()]),
        ),
        DocumentError::Json(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([e.to_string()]),
        ),
        DocumentError::Crm(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([e.to_string()]),
        ),
        DocumentError::Invoice(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([e.to_string()]),
        ),
        DocumentError::Storage(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Envelope::err([e.to_string()]),
        ),
    }
}

pub async fn attach(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<AttachBody>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(body.company);
    let path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    let party_id = body.party_id.map(PartyId::new);
    let invoice_id = body.invoice_id.map(InvoiceId::new);
    let content = if let Some(b64) = body.content_base64 {
        Some(
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64.trim())
                .map_err(|_| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(Envelope::err([
                            "content_base64 must be valid base64".to_string()
                        ])),
                    )
                })?,
        )
    } else {
        None
    };
    let doc = attach_document(
        &path,
        body.kind,
        body.path_hint,
        party_id,
        invoice_id,
        body.notes,
        content.as_deref(),
    )
    .await
    .map_err(|e| {
        let (s, env) = map_doc(e);
        (s, Json(env))
    })?;
    Ok(Json(Envelope::ok(serde_json::to_value(doc).unwrap())))
}

pub async fn list_docs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<DocumentListQuery>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(query.company);
    let path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    if let Some(raw_id) = query.document_id {
        let id = DocumentId::new(raw_id);
        let doc = get_document(&path, &id).map_err(|e| {
            let (s, env) = map_doc(e);
            (s, Json(env))
        })?;
        let doc = doc.ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(Envelope::err([format!("document not found: {id}")])),
            )
        })?;
        return Ok(Json(Envelope::ok(serde_json::to_value(doc).unwrap())));
    }
    let docs = list_documents(&path).map_err(|e| {
        let (s, env) = map_doc(e);
        (s, Json(env))
    })?;
    Ok(Json(Envelope::ok(serde_json::to_value(docs).unwrap())))
}

pub async fn delete_doc(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DeleteBody>,
) -> Result<Json<Envelope<Value>>, (StatusCode, Json<Envelope<Value>>)> {
    let actor = parse_actor(&headers).map_err(|(s, e)| (s, Json(e)))?;
    let company = PathBuf::from(body.company);
    let path = authorize_company(&state.allowlist_root, &company, &actor)
        .await
        .map_err(|e| {
            let (s, env) = map_core(e);
            (s, Json(env))
        })?;
    let id = DocumentId::new(body.document_id);
    let doc = remove_document(&path, &id, body.delete_object)
        .await
        .map_err(|e| {
            let (s, env) = map_doc(e);
            (s, Json(env))
        })?;
    Ok(Json(Envelope::ok(serde_json::to_value(doc).unwrap())))
}
