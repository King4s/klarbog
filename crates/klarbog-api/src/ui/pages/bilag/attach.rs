//! Bilag multipart upload — attach a document (object store put + metadata).

use axum::extract::{Multipart, State};
use axum::http::HeaderMap;
use axum::response::Response;
use klarbog_plugin_documents::{attach_document, DocumentKind};
use klarbog_plugin_invoice::InvoiceId;
use klarbog_types::PartyId;

use super::super::common::{authorize_company, company_from, html_ok};
use super::view::bilag_page;
use crate::AppState;

fn parse_kind(raw: &str) -> DocumentKind {
    match raw.trim() {
        "receipt" => DocumentKind::Receipt,
        "invoice_scan" => DocumentKind::InvoiceScan,
        _ => DocumentKind::Other,
    }
}

fn none_if_empty(s: String) -> Option<String> {
    let t = s.trim().to_string();
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}

pub async fn bilag_attach(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    let company = company_from(&headers);
    let err_page =
        |e: String| async { html_ok(bilag_page(&state, &company, false, String::new(), e).await) };
    if company.is_empty() {
        return html_ok(
            bilag_page(
                &state,
                &company,
                false,
                String::new(),
                "Sæt firmasti under Indstillinger.".into(),
            )
            .await,
        );
    }
    let path = match authorize_company(&state, &company).await {
        Ok(p) => p,
        Err(e) => return err_page(e).await,
    };

    let mut kind = DocumentKind::Other;
    let mut path_hint = String::new();
    let mut party_id: Option<String> = None;
    let mut invoice_id: Option<String> = None;
    let mut notes: Option<String> = None;
    let mut content: Option<Vec<u8>> = None;

    loop {
        let field = match multipart.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => return err_page(format!("Upload-fejl: {e}")).await,
        };
        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "kind" => kind = parse_kind(&field.text().await.unwrap_or_default()),
            "path_hint" => path_hint = field.text().await.unwrap_or_default().trim().to_string(),
            "party_id" => party_id = none_if_empty(field.text().await.unwrap_or_default()),
            "invoice_id" => invoice_id = none_if_empty(field.text().await.unwrap_or_default()),
            "notes" => notes = none_if_empty(field.text().await.unwrap_or_default()),
            "file" => {
                let file_name = field.file_name().unwrap_or_default().to_string();
                match field.bytes().await {
                    Ok(b) if !b.is_empty() => {
                        if path_hint.is_empty() && !file_name.is_empty() {
                            path_hint = format!("bilag/{file_name}");
                        }
                        content = Some(b.to_vec());
                    }
                    Ok(_) => {}
                    Err(e) => return err_page(format!("Fil-læsning fejlede: {e}")).await,
                }
            }
            _ => {}
        }
    }

    match attach_document(
        &path,
        kind,
        path_hint,
        party_id.map(PartyId::new),
        invoice_id.map(InvoiceId::new),
        notes,
        content.as_deref(),
    )
    .await
    {
        Ok(doc) => html_ok(
            bilag_page(
                &state,
                &company,
                false,
                format!("Bilag vedhæftet: {} · {}", doc.id, doc.path_hint),
                String::new(),
            )
            .await,
        ),
        Err(e) => err_page(e.to_string()).await,
    }
}
