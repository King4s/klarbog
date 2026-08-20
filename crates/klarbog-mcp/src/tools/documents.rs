//! Documents + exceptions MCP tools — mirror HTTP `/api/v1/documents` and `/api/v1/exceptions`.
//! No journal write (ADR-004).

use super::auth::{authorize_company, map_core_error, parse_actor, parse_company};
use klarbog_plugin_documents::{
    attach_document, get_document, get_exception, list_documents, list_exceptions, raise_exception,
    remove_document, set_exception_open, DocumentError, DocumentId, DocumentKind, ExceptionId,
    ExceptionSeverity,
};
use klarbog_plugin_invoice::InvoiceId;
use klarbog_types::{Envelope, PartyId};
use serde_json::Value;
use std::path::Path;

fn map_doc(err: DocumentError) -> Envelope<Value> {
    Envelope::err([err.to_string()])
}

fn parse_kind(args: &Value) -> Result<DocumentKind, String> {
    let raw = args
        .get("kind")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing kind".to_string())?;
    serde_json::from_value(Value::String(raw.to_string()))
        .map_err(|_| format!("invalid kind: {raw} (receipt|invoice_scan|other)"))
}

fn parse_severity(args: &Value) -> Result<ExceptionSeverity, String> {
    let raw = args
        .get("severity")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing severity".to_string())?;
    serde_json::from_value(Value::String(raw.to_string()))
        .map_err(|_| format!("invalid severity: {raw} (info|warn|error)"))
}

/// Mirror POST /api/v1/documents — attach metadata (+ optional content_base64); no journal write.
pub async fn documents_attach(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let company = match parse_company(args) {
        Ok(c) => c,
        Err(e) => return Envelope::err([e]),
    };
    let kind = match parse_kind(args) {
        Ok(k) => k,
        Err(e) => return Envelope::err([e]),
    };
    let path_hint = match args.get("path_hint").and_then(|v| v.as_str()) {
        Some(p) => p.to_string(),
        None => return Envelope::err(["missing path_hint"]),
    };
    let party_id = args
        .get("party_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| PartyId::new(s.to_string()));
    let invoice_id = args
        .get("invoice_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| InvoiceId::new(s.to_string()));
    let notes = args
        .get("notes")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let content = if let Some(b64) = args.get("content_base64").and_then(|v| v.as_str()) {
        match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64.trim()) {
            Ok(bytes) => Some(bytes),
            Err(_) => {
                return Envelope::err(["content_base64 must be valid base64".to_string()]);
            }
        }
    } else {
        None
    };
    let path = match authorize_company(allowlist_root, &company, &actor).await {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    match attach_document(
        &path,
        kind,
        path_hint,
        party_id,
        invoice_id,
        notes,
        content.as_deref(),
    )
    .await
    {
        Ok(doc) => Envelope::ok(serde_json::to_value(doc).unwrap()),
        Err(e) => map_doc(e),
    }
}

/// Mirror GET /api/v1/documents — list all, or one when `document_id` set.
pub async fn documents_list(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let company = match parse_company(args) {
        Ok(c) => c,
        Err(e) => return Envelope::err([e]),
    };
    let path = match authorize_company(allowlist_root, &company, &actor).await {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    if let Some(raw_id) = args
        .get("document_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        let id = DocumentId::new(raw_id.to_string());
        return match get_document(&path, &id) {
            Ok(Some(doc)) => Envelope::ok(serde_json::to_value(doc).unwrap()),
            Ok(None) => Envelope::err([format!("document not found: {id}")]),
            Err(e) => map_doc(e),
        };
    }
    match list_documents(&path) {
        Ok(docs) => Envelope::ok(serde_json::to_value(docs).unwrap()),
        Err(e) => map_doc(e),
    }
}

/// Mirror DELETE /api/v1/documents — remove metadata; optional object delete (default true).
pub async fn documents_delete(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let company = match parse_company(args) {
        Ok(c) => c,
        Err(e) => return Envelope::err([e]),
    };
    let document_id = match args.get("document_id").and_then(|v| v.as_str()) {
        Some(id) if !id.is_empty() => DocumentId::new(id.to_string()),
        _ => return Envelope::err(["missing document_id"]),
    };
    let delete_object = args
        .get("delete_object")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let path = match authorize_company(allowlist_root, &company, &actor).await {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    match remove_document(&path, &document_id, delete_object).await {
        Ok(doc) => Envelope::ok(serde_json::to_value(doc).unwrap()),
        Err(e) => map_doc(e),
    }
}

/// Mirror POST /api/v1/exceptions — raise open exception; no journal write.
pub async fn exceptions_raise(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let company = match parse_company(args) {
        Ok(c) => c,
        Err(e) => return Envelope::err([e]),
    };
    let code = match args.get("code").and_then(|v| v.as_str()) {
        Some(c) => c.to_string(),
        None => return Envelope::err(["missing code"]),
    };
    let severity = match parse_severity(args) {
        Ok(s) => s,
        Err(e) => return Envelope::err([e]),
    };
    let message = match args.get("message").and_then(|v| v.as_str()) {
        Some(m) => m.to_string(),
        None => return Envelope::err(["missing message"]),
    };
    let related_ids = args
        .get("related_ids")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let path = match authorize_company(allowlist_root, &company, &actor).await {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    match raise_exception(&path, code, severity, message, related_ids) {
        Ok(exc) => Envelope::ok(serde_json::to_value(exc).unwrap()),
        Err(e) => map_doc(e),
    }
}

/// Mirror GET /api/v1/exceptions — list (open_only default true), or one when `exception_id` set.
pub async fn exceptions_list(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let company = match parse_company(args) {
        Ok(c) => c,
        Err(e) => return Envelope::err([e]),
    };
    let open_only = args
        .get("open_only")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let path = match authorize_company(allowlist_root, &company, &actor).await {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    if let Some(raw_id) = args
        .get("exception_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        let id = ExceptionId::new(raw_id.to_string());
        return match get_exception(&path, &id) {
            Ok(Some(exc)) => Envelope::ok(serde_json::to_value(exc).unwrap()),
            Ok(None) => Envelope::err([format!("exception not found: {id}")]),
            Err(e) => map_doc(e),
        };
    }
    match list_exceptions(&path, open_only) {
        Ok(items) => Envelope::ok(serde_json::to_value(items).unwrap()),
        Err(e) => map_doc(e),
    }
}

/// Mirror PATCH /api/v1/exceptions — set `open` (close with open:false).
pub async fn exceptions_set_open(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let company = match parse_company(args) {
        Ok(c) => c,
        Err(e) => return Envelope::err([e]),
    };
    let exception_id = match args.get("exception_id").and_then(|v| v.as_str()) {
        Some(id) if !id.is_empty() => ExceptionId::new(id.to_string()),
        _ => return Envelope::err(["missing exception_id"]),
    };
    let open = args.get("open").and_then(|v| v.as_bool()).unwrap_or(false);
    let path = match authorize_company(allowlist_root, &company, &actor).await {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    match set_exception_open(&path, &exception_id, open) {
        Ok(exc) => Envelope::ok(serde_json::to_value(exc).unwrap()),
        Err(e) => map_doc(e),
    }
}

#[cfg(test)]
#[path = "documents_tests.rs"]
mod tests;
