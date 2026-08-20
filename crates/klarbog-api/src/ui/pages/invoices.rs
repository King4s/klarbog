//! Invoices SSR — list, draft create, lifecycle preview actions.

use askama::Template;
use axum::extract::{Form, State};
use axum::http::HeaderMap;
use axum::response::Response;
use klarbog_core::journal_preview;
use klarbog_plugin_crm::list_parties;
use klarbog_plugin_invoice::{
    create_draft_from_new, list_invoices, mark_paid_preview, mark_part_paid_preview, patch_status,
    InvoiceConfig, InvoiceId, InvoiceKind, InvoiceStatus, NewLine,
};
use klarbog_types::{Actor, PartyId};
use serde::Deserialize;

use super::common::{authorize_company, company_from, foot, format_dkk, html_ok, nav, ACTOR};
use crate::AppState;

struct InvoiceRow {
    id: String,
    status: String,
    total: String,
    party_id: String,
    can_send: bool,
    can_collect: bool,
}

struct PartyOption {
    id: String,
    label: String,
}

#[derive(Template)]
#[template(path = "invoices.html")]
struct InvoicesTemplate {
    title: &'static str,
    nav_home: bool,
    nav_parties: bool,
    nav_invoices: bool,
    nav_bank: bool,
    nav_bilag: bool,
    nav_journal: bool,
    nav_chart: bool,
    nav_settings: bool,
    foot: String,
    has_flash_ok: bool,
    flash_ok: String,
    has_flash_err: bool,
    flash_err: String,
    has_company: bool,
    company: String,
    invoices: Vec<InvoiceRow>,
    parties: Vec<PartyOption>,
}

fn status_label(s: InvoiceStatus) -> &'static str {
    match s {
        InvoiceStatus::Draft => "draft",
        InvoiceStatus::Sent => "sent",
        InvoiceStatus::PartPaid => "part_paid",
        InvoiceStatus::Paid => "paid",
        InvoiceStatus::Void => "void",
    }
}

async fn load_page(
    state: &AppState,
    company: &str,
    flash_ok: String,
    flash_err: String,
) -> InvoicesTemplate {
    let n = nav("invoices");
    let base = InvoicesTemplate {
        title: "Fakturaer",
        nav_home: n.home,
        nav_parties: n.parties,
        nav_invoices: n.invoices,
        nav_bank: n.bank,
        nav_bilag: n.bilag,
        nav_journal: n.journal,
        nav_chart: n.chart,
        nav_settings: n.settings,
        foot: foot(state),
        has_flash_ok: !flash_ok.is_empty(),
        flash_ok,
        has_flash_err: !flash_err.is_empty(),
        flash_err,
        has_company: !company.is_empty(),
        company: company.to_string(),
        invoices: Vec::new(),
        parties: Vec::new(),
    };
    if company.is_empty() {
        return base;
    }
    let path = match authorize_company(state, company).await {
        Ok(p) => p,
        Err(e) => {
            return InvoicesTemplate {
                has_flash_err: true,
                flash_err: e,
                ..base
            };
        }
    };
    let parties = list_parties(&path)
        .unwrap_or_default()
        .into_iter()
        .map(|p| PartyOption {
            id: p.id.to_string(),
            label: format!("{} ({})", p.display_name, p.id),
        })
        .collect();
    let invoices = list_invoices(&path)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|inv| {
            let total = inv.total_minor().ok()?;
            Some(InvoiceRow {
                id: inv.id.to_string(),
                status: status_label(inv.status).into(),
                total: format_dkk(total),
                party_id: inv.party_id.to_string(),
                can_send: inv.status == InvoiceStatus::Draft,
                can_collect: inv.status.allows_mark_paid(),
            })
        })
        .collect();
    InvoicesTemplate {
        parties,
        invoices,
        ..base
    }
}

pub async fn invoices_get(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let company = company_from(&headers);
    html_ok(load_page(&state, &company, String::new(), String::new()).await)
}

#[derive(Deserialize)]
pub struct InvoiceActionForm {
    pub action: String,
    pub party_id: Option<String>,
    pub kind: Option<String>,
    pub description: Option<String>,
    pub amount_minor: Option<String>,
    pub invoice_id: Option<String>,
    pub part_amount_minor: Option<String>,
}

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
