//! Invoice POST actions (create / send / paid preview / part paid preview).

use axum::extract::{Form, State};
use axum::http::HeaderMap;
use axum::response::Response;
use klarbog_core::journal_preview;
use klarbog_plugin_invoice::{
    create_draft_from_new, mark_paid_preview, mark_part_paid_preview, patch_status, InvoiceConfig,
    InvoiceId, InvoiceKind, InvoiceStatus, NewLine,
};
use klarbog_types::{Actor, PartyId};

use super::super::common::{authorize_company, company_from, html_ok, ACTOR};
use super::form::InvoiceActionForm;
use super::view::load_page;
use crate::AppState;

pub async fn invoices_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<InvoiceActionForm>,
) -> Response {
    let company = company_from(&headers);
    if company.is_empty() {
        return html_ok(
            load_page(
                &state,
                &company,
                String::new(),
                "Sæt firmasti under Indstillinger.".into(),
            )
            .await,
        );
    }
    let path = match authorize_company(&state, &company).await {
        Ok(p) => p,
        Err(e) => {
            return html_ok(load_page(&state, &company, String::new(), e).await);
        }
    };
    let actor = Actor::user(ACTOR);
    let action = form.action.trim();
    match action {
        "create" => {
            let party_raw = form.party_id.unwrap_or_default();
            let desc = form.description.unwrap_or_default().trim().to_string();
            let amount_raw = form.amount_minor.unwrap_or_default();
            let amount: i64 = match amount_raw.trim().parse() {
                Ok(v) if v > 0 => v,
                _ => {
                    return html_ok(
                        load_page(
                            &state,
                            &company,
                            String::new(),
                            "Beløb skal være positivt heltal (øre).".into(),
                        )
                        .await,
                    );
                }
            };
            let kind = match form.kind.as_deref() {
                Some("purchase") => InvoiceKind::Purchase,
                _ => InvoiceKind::Sale,
            };
            match create_draft_from_new(
                &path,
                PartyId::new(party_raw),
                kind,
                vec![NewLine {
                    description: desc,
                    amount_minor: amount,
                    currency: "DKK".into(),
                }],
            ) {
                Ok(inv) => html_ok(
                    load_page(
                        &state,
                        &company,
                        format!("Kladde oprettet: {}", inv.id),
                        String::new(),
                    )
                    .await,
                ),
                Err(e) => html_ok(load_page(&state, &company, String::new(), e.to_string()).await),
            }
        }
        "send" => {
            let id = InvoiceId::new(form.invoice_id.unwrap_or_default());
            match patch_status(&path, &id, InvoiceStatus::Sent) {
                Ok(_) => html_ok(
                    load_page(
                        &state,
                        &company,
                        format!("{id} sat til sent"),
                        String::new(),
                    )
                    .await,
                ),
                Err(e) => html_ok(load_page(&state, &company, String::new(), e.to_string()).await),
            }
        }
        "paid_preview" => {
            let id = InvoiceId::new(form.invoice_id.unwrap_or_default());
            match mark_paid_preview(&path, &id, &actor, &InvoiceConfig::default()) {
                Ok((inv, entry)) => {
                    let preview = journal_preview(
                        &state.allowlist_root,
                        std::path::Path::new(&company),
                        &entry,
                        &actor,
                        &state.confirm,
                        &state.registry,
                    )
                    .await;
                    match preview {
                        Ok(p) => html_ok(
                            load_page(
                                &state,
                                &company,
                                format!(
                                    "Betalt preview for {} · token {} · memo {}",
                                    inv.id, p.confirm_token.token, entry.memo
                                ),
                                String::new(),
                            )
                            .await,
                        ),
                        Err(e) => {
                            html_ok(load_page(&state, &company, String::new(), e.to_string()).await)
                        }
                    }
                }
                Err(e) => html_ok(load_page(&state, &company, String::new(), e.to_string()).await),
            }
        }
        "part_paid_preview" => {
            let id = InvoiceId::new(form.invoice_id.unwrap_or_default());
            let amount: i64 = match form.part_amount_minor.unwrap_or_default().trim().parse() {
                Ok(v) if v > 0 => v,
                _ => {
                    return html_ok(
                        load_page(
                            &state,
                            &company,
                            String::new(),
                            "Delbetaling skal være positivt heltal (øre).".into(),
                        )
                        .await,
                    );
                }
            };
            match mark_part_paid_preview(&path, &id, amount, &actor, &InvoiceConfig::default()) {
                Ok((inv, entry)) => {
                    let preview = journal_preview(
                        &state.allowlist_root,
                        std::path::Path::new(&company),
                        &entry,
                        &actor,
                        &state.confirm,
                        &state.registry,
                    )
                    .await;
                    match preview {
                        Ok(p) => html_ok(
                            load_page(
                                &state,
                                &company,
                                format!(
                                    "Delbetalt preview for {} · token {} · memo {}",
                                    inv.id, p.confirm_token.token, entry.memo
                                ),
                                String::new(),
                            )
                            .await,
                        ),
                        Err(e) => {
                            html_ok(load_page(&state, &company, String::new(), e.to_string()).await)
                        }
                    }
                }
                Err(e) => html_ok(load_page(&state, &company, String::new(), e.to_string()).await),
            }
        }
        _ => html_ok(
            load_page(
                &state,
                &company,
                String::new(),
                format!("Ukendt handling: {action}"),
            )
            .await,
        ),
    }
}
