//! Bank SSR — CSV import preview and reconcile suggest.

use askama::Template;
use axum::extract::{Form, State};
use axum::http::HeaderMap;
use axum::response::Response;
use chrono::{DateTime, NaiveDate, Utc};
use klarbog_plugin_bank::{
    default_source_for_rail, import_preview, suggest_matches, BankImportConfig, BankProfile,
    BankRow,
};
use klarbog_types::{Actor, Currency, MinorAmount};
use serde::Deserialize;

use super::common::{authorize_company, company_from, foot, format_dkk, html_ok, nav, ACTOR};
use crate::AppState;

struct DraftRow {
    idx: usize,
    date: String,
    memo: String,
    dkk: String,
    minor: String,
}

struct SuggestRow {
    date: String,
    text: String,
    amount: String,
    best_invoice: String,
    confidence_bps: String,
    unsafe_reason: String,
}

#[derive(Template)]
#[template(path = "bank.html")]
struct BankTemplate {
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
    provider: String,
    csv: String,
    row_date: String,
    row_text: String,
    row_amount: String,
    has_import: bool,
    import_count: String,
    import_source: String,
    drafts: Vec<DraftRow>,
    has_suggest: bool,
    suggest_count: String,
    suggestions: Vec<SuggestRow>,
}

struct BankView {
    company: String,
    provider: String,
    csv: String,
    row_date: String,
    row_text: String,
    row_amount: String,
    drafts: Vec<DraftRow>,
    import_source: String,
    suggestions: Vec<SuggestRow>,
    flash_ok: String,
    flash_err: String,
}

fn bank_page(state: &AppState, v: BankView) -> BankTemplate {
    let n = nav("bank");
    BankTemplate {
        title: "Bank",
        nav_home: n.home,
        nav_parties: n.parties,
        nav_invoices: n.invoices,
        nav_bank: n.bank,
        nav_bilag: n.bilag,
        nav_journal: n.journal,
        nav_chart: n.chart,
        nav_settings: n.settings,
        foot: foot(state),
        has_flash_ok: !v.flash_ok.is_empty(),
        flash_ok: v.flash_ok,
        has_flash_err: !v.flash_err.is_empty(),
        flash_err: v.flash_err,
        has_company: !v.company.is_empty(),
        company: v.company,
        provider: v.provider,
        csv: v.csv,
        row_date: v.row_date,
        row_text: v.row_text,
        row_amount: v.row_amount,
        has_import: !v.drafts.is_empty(),
        import_count: v.drafts.len().to_string(),
        import_source: v.import_source,
        drafts: v.drafts,
        has_suggest: !v.suggestions.is_empty(),
        suggest_count: v.suggestions.len().to_string(),
        suggestions: v.suggestions,
    }
}

pub async fn bank_get(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let company = company_from(&headers);
    html_ok(bank_page(
        &state,
        BankView {
            company,
            provider: "generic_dk".into(),
            csv: String::new(),
            row_date: "2026-05-20".into(),
            row_text: "Customer payment".into(),
            row_amount: "50000".into(),
            drafts: Vec::new(),
            import_source: String::new(),
            suggestions: Vec::new(),
            flash_ok: String::new(),
            flash_err: String::new(),
        },
    ))
}

#[derive(Deserialize)]
pub struct BankActionForm {
    pub action: String,
    pub provider: String,
    pub csv: String,
    pub row_date: String,
    pub row_text: String,
    pub row_amount: String,
}

fn parse_provider(raw: &str) -> BankProfile {
    match raw.trim() {
        "revolut" => BankProfile::Revolut,
        "stripe" => BankProfile::Stripe,
        _ => BankProfile::GenericDk,
    }
}

fn provider_name(p: BankProfile) -> &'static str {
    match p {
        BankProfile::GenericDk => "generic_dk",
        BankProfile::Revolut => "revolut",
        BankProfile::Stripe => "stripe",
    }
}

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
                drafts: Vec::new(),
                import_source: String::new(),
                suggestions: Vec::new(),
                flash_ok,
                flash_err,
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
                            suggestions: Vec::new(),
                            flash_ok: "Import-preview ok".into(),
                            flash_err: String::new(),
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
                            SuggestRow {
                                date: m.date,
                                text: m.text,
                                amount: format_dkk(m.amount_minor),
                                best_invoice: best
                                    .map(|s| s.invoice_id.clone())
                                    .unwrap_or_default(),
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
                            drafts: Vec::new(),
                            import_source: String::new(),
                            suggestions,
                            flash_ok: "Afstem-forslag klar".into(),
                            flash_err: String::new(),
                        },
                    ))
                }
                Err(e) => html_ok(empty(String::new(), e.to_string())),
            }
        }
        _ => html_ok(empty(
            String::new(),
            format!("Ukendt handling: {}", form.action),
        )),
    }
}
