//! Bank POST — import preview, reconcile suggest, apply→journal preview.

use axum::extract::{Form, State};
use axum::http::HeaderMap;
use axum::response::Response;
use chrono::{DateTime, NaiveDate, Utc};
use klarbog_core::journal_preview;
use klarbog_plugin_bank::{
    apply_match, default_source_for_rail, import_preview, suggest_matches, BankImportConfig,
    BankRow,
};
use klarbog_types::{Actor, Currency, MinorAmount};

use super::super::common::{authorize_company, company_from, format_dkk, html_ok, ACTOR};
use super::form::{
    parse_provider, provider_name, wants_force, BankActionForm, DraftRow, SuggestRow,
};
use super::view::{bank_page, BankView};
use crate::AppState;

pub async fn bank_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<BankActionForm>,
) -> Response {
    let company = company_from(&headers);
    let provider = parse_provider(&form.provider);
    let provider_s = provider_name(provider).to_string();
    let empty = |flash_ok: String, flash_err: String| {
        bank_page(
            &state,
            BankView {
                company: company.clone(),
                provider: provider_s.clone(),
                csv: form.csv.clone(),
                row_date: form.row_date.clone(),
                row_text: form.row_text.clone(),
                row_amount: form.row_amount.clone(),
                flash_ok,
                flash_err,
                ..Default::default()
            },
        )
    };
    if company.is_empty() {
        return html_ok(empty(
            String::new(),
            "Sæt firmasti under Indstillinger.".into(),
        ));
    }
    let path = match authorize_company(&state, &company).await {
        Ok(p) => p,
        Err(e) => return html_ok(empty(String::new(), e)),
    };
    let actor = Actor::user(ACTOR);
    let currency = Currency::new("DKK").unwrap();
    let cfg = BankImportConfig {
        currency: currency.clone(),
        ..BankImportConfig::default()
    };
    match form.action.trim() {
        "import_preview" => {
            let source = default_source_for_rail(provider);
            if form.csv.trim().is_empty() {
                return html_ok(empty(
                    String::new(),
                    "CSV tekst kræves til import-preview.".into(),
                ));
            }
            match import_preview(
                source,
                provider,
                Some(form.csv.trim()),
                &cfg,
                &actor,
                Some(&path),
            )
            .await
            {
                Ok((rows, drafts)) => {
                    let draft_rows: Vec<DraftRow> = rows
                        .iter()
                        .zip(drafts.iter())
                        .enumerate()
                        .map(|(idx, (row, entry))| DraftRow {
                            idx,
                            date: row.date.format("%Y-%m-%d").to_string(),
                            memo: entry.memo.clone(),
                            dkk: format_dkk(row.amount_minor.minor()),
                            minor: row.amount_minor.minor().to_string(),
                        })
                        .collect();
                    html_ok(bank_page(
                        &state,
                        BankView {
                            company,
                            provider: provider_s,
                            csv: form.csv,
                            row_date: form.row_date,
                            row_text: form.row_text,
                            row_amount: form.row_amount,
                            drafts: draft_rows,
                            import_source: format!("{source:?}"),
                            flash_ok: "Import-preview ok".into(),
                            ..Default::default()
                        },
                    ))
                }
                Err(e) => html_ok(empty(String::new(), e.to_string())),
            }
        }
        "reconcile_suggest" => {
            let amount: i64 = match form.row_amount.trim().parse() {
                Ok(v) => v,
                Err(_) => {
                    return html_ok(empty(String::new(), "Beløb skal være heltal (øre).".into()));
                }
            };
            let date = match NaiveDate::parse_from_str(form.row_date.trim(), "%Y-%m-%d") {
                Ok(d) => d,
                Err(e) => {
                    return html_ok(empty(String::new(), format!("Ugyldig dato: {e}")));
                }
            };
            let dt: DateTime<Utc> = date.and_hms_opt(0, 0, 0).unwrap().and_utc();
            let rows = vec![BankRow {
                date: dt,
                text: form.row_text.trim().to_string(),
                amount_minor: MinorAmount::from_minor(amount),
            }];
            match suggest_matches(&path, &rows) {
                Ok(matches) => {
                    let suggestions: Vec<SuggestRow> = matches
                        .into_iter()
                        .map(|m| {
                            let best = m.suggestions.first();
                            let best_invoice =
                                best.map(|s| s.invoice_id.clone()).unwrap_or_default();
                            SuggestRow {
                                date: m.date,
                                text: m.text,
                                amount: format_dkk(m.amount_minor),
                                can_apply: !best_invoice.is_empty(),
                                best_invoice,
                                confidence_bps: best
                                    .map(|s| s.confidence_bps.to_string())
                                    .unwrap_or_else(|| "—".into()),
                                unsafe_reason: m.unsafe_match_reason.unwrap_or_default(),
                            }
                        })
                        .collect();
                    html_ok(bank_page(
                        &state,
                        BankView {
                            company,
                            provider: provider_s,
                            csv: form.csv,
                            row_date: form.row_date,
                            row_text: form.row_text,
                            row_amount: form.row_amount,
                            suggestions,
                            flash_ok: "Afstem-forslag klar".into(),
                            ..Default::default()
                        },
                    ))
                }
                Err(e) => html_ok(empty(String::new(), e.to_string())),
            }
        }
        "apply_preview" => {
            let invoice_id = form.invoice_id.trim();
            if invoice_id.is_empty() {
                return html_ok(empty(String::new(), "invoice_id kræves til apply.".into()));
            }
            let amount: i64 = match form.row_amount.trim().parse() {
                Ok(v) => v,
                Err(_) => {
                    return html_ok(empty(String::new(), "Beløb skal være heltal (øre).".into()));
                }
            };
            let date = match NaiveDate::parse_from_str(form.row_date.trim(), "%Y-%m-%d") {
                Ok(d) => d,
                Err(e) => {
                    return html_ok(empty(String::new(), format!("Ugyldig dato: {e}")));
                }
            };
            let row_index: usize = form.row_index.trim().parse().unwrap_or(0);
            let dt: DateTime<Utc> = date.and_hms_opt(0, 0, 0).unwrap().and_utc();
            let bank_row = BankRow {
                date: dt,
                text: form.row_text.trim().to_string(),
                amount_minor: MinorAmount::from_minor(amount),
            };
            let force = wants_force(&form.force);
            match apply_match(&path, &bank_row, invoice_id, &actor, force, row_index) {
                Ok(applied) => {
                    match journal_preview(
                        &state.allowlist_root,
                        std::path::Path::new(&company),
                        &applied.entry,
                        &actor,
                        &state.confirm,
                        &state.registry,
                    )
                    .await
                    {
                        Ok(p) => {
                            let entry_json = match serde_json::to_string(&applied.entry) {
                                Ok(j) => j,
                                Err(e) => {
                                    return html_ok(empty(String::new(), e.to_string()));
                                }
                            };
                            html_ok(bank_page(
                                &state,
                                BankView {
                                    company,
                                    provider: provider_s,
                                    csv: form.csv,
                                    row_date: form.row_date,
                                    row_text: form.row_text,
                                    row_amount: form.row_amount,
                                    flash_ok: format!(
                                        "Apply-preview ok · inv {} · {} bps{}",
                                        invoice_id,
                                        applied.confidence_bps,
                                        if applied.forced { " (forced)" } else { "" },
                                    ),
                                    pending_label: format!(
                                        "inv {} · {}",
                                        invoice_id, applied.entry.memo
                                    ),
                                    pending_entry_json: entry_json,
                                    pending_token: p.confirm_token.token,
                                    pending_invoice_id: invoice_id.to_string(),
                                    ..Default::default()
                                },
                            ))
                        }
                        Err(e) => html_ok(empty(String::new(), e.to_string())),
                    }
                }
                Err(e) => html_ok(empty(String::new(), e.to_string())),
            }
        }
        "commit_apply" => {
            match super::commit::commit_apply(
                &state,
                &company,
                &path,
                &actor,
                &form.invoice_id,
                &form.entry_json,
                &form.confirm_token,
            )
            .await
            {
                Ok(ok) => html_ok(empty(ok, String::new())),
                Err(e) => html_ok(empty(String::new(), e)),
            }
        }
        _ => html_ok(empty(
            String::new(),
            format!("Ukendt handling: {}", form.action),
        )),
    }
}
