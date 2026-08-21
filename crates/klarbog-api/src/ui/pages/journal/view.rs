//! Journal page render helpers.

use askama::Template;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;

use super::super::common::{authorize_company, company_from, foot, format_dkk, html_ok, nav};
use super::form::JournalFields;
use crate::AppState;
use klarbog_plugin_rules_dk::chart_accounts;

/// One `<option>` in an account dropdown.
pub(super) struct AccountOption {
    pub value: String,
    pub text: String,
    pub selected: bool,
}

/// Chart dropdown options with `current` preselected. A value outside the
/// chart (legacy data) is appended as its own option so re-rendering never
/// silently swaps the account.
fn account_options(current: &str) -> Vec<AccountOption> {
    let mut options: Vec<AccountOption> = chart_accounts()
        .iter()
        .map(|a| AccountOption {
            value: a.code.to_string(),
            text: format!("{} · {}", a.code, a.label),
            selected: a.code.to_string() == current,
        })
        .collect();
    if !current.is_empty() && !options.iter().any(|o| o.selected) {
        options.push(AccountOption {
            value: current.to_string(),
            text: format!("{current} (uden for kontoplan)"),
            selected: true,
        });
    }
    options
}

/// One leg per row; entry columns (incl. reversal id) only on the first leg.
pub(super) struct PostingRow {
    pub id: String,
    pub date: String,
    pub memo: String,
    pub account: String,
    pub debit: String,
    pub credit: String,
}

#[derive(Template)]
#[template(path = "journal.html")]
pub(super) struct JournalTemplate {
    pub title: &'static str,
    pub nav_home: bool,
    pub nav_parties: bool,
    pub nav_invoices: bool,
    pub nav_bank: bool,
    pub nav_bilag: bool,
    pub nav_journal: bool,
    pub nav_chart: bool,
    pub nav_settings: bool,
    pub foot: String,
    pub has_flash_ok: bool,
    pub flash_ok: String,
    pub has_flash_err: bool,
    pub flash_err: String,
    pub has_company: bool,
    pub company: String,
    pub memo: String,
    pub account1: String,
    pub direction1: String,
    pub amount1: String,
    pub account2: String,
    pub direction2: String,
    pub amount2: String,
    pub moms_gross: String,
    pub moms_memo: String,
    pub has_moms: bool,
    pub moms_suggested: bool,
    pub moms_reason: String,
    pub moms_gross_minor: String,
    pub moms_net_minor: String,
    pub moms_vat_minor: String,
    pub moms_rate_bps: String,
    pub has_preview: bool,
    pub confirm_token: String,
    pub expires_unix_ms: String,
    pub payload_digest: String,
    pub entry_json: String,
    pub postings: Vec<PostingRow>,
    pub account1_options: Vec<AccountOption>,
    pub account2_options: Vec<AccountOption>,
}

pub(super) fn journal_page(
    state: &AppState,
    company: String,
    fields: JournalFields,
    flash_ok: String,
    flash_err: String,
) -> JournalTemplate {
    let n = nav("journal");
    let account1_options = account_options(&fields.account1);
    let account2_options = account_options(&fields.account2);
    JournalTemplate {
        title: "Journal",
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
        company,
        memo: fields.memo,
        account1: fields.account1,
        direction1: fields.direction1,
        amount1: fields.amount1,
        account2: fields.account2,
        direction2: fields.direction2,
        amount2: fields.amount2,
        moms_gross: fields.moms_gross,
        moms_memo: fields.moms_memo,
        has_moms: fields.has_moms,
        moms_suggested: fields.moms_suggested,
        moms_reason: fields.moms_reason,
        moms_gross_minor: fields.moms_gross_minor,
        moms_net_minor: fields.moms_net_minor,
        moms_vat_minor: fields.moms_vat_minor,
        moms_rate_bps: fields.moms_rate_bps,
        has_preview: fields.has_preview,
        confirm_token: fields.confirm_token,
        expires_unix_ms: fields.expires_unix_ms,
        payload_digest: fields.payload_digest,
        entry_json: fields.entry_json,
        postings: Vec::new(),
        account1_options,
        account2_options,
    }
}

/// Flatten recent posted entries to leg rows (entry info on first leg only).
async fn load_postings(path: &std::path::Path, limit: i64) -> Result<Vec<PostingRow>, String> {
    let company = klarbog_core::open_existing(path)
        .await
        .map_err(|e| e.to_string())?;
    let entries = company
        .recent_entries(limit)
        .await
        .map_err(|e| e.to_string())?;
    let mut rows = Vec::new();
    for e in entries {
        let date = e.as_of.chars().take(10).collect::<String>();
        for (i, leg) in e.legs.iter().enumerate() {
            let (debit, credit) = match leg.direction.as_str() {
                "debit" => (format_dkk(leg.amount_minor), String::new()),
                _ => (String::new(), format_dkk(leg.amount_minor)),
            };
            rows.push(PostingRow {
                id: if i == 0 { e.id.clone() } else { String::new() },
                date: if i == 0 { date.clone() } else { String::new() },
                memo: if i == 0 {
                    e.memo.clone()
                } else {
                    String::new()
                },
                account: leg.account.clone(),
                debit,
                credit,
            });
        }
    }
    Ok(rows)
}

pub async fn journal_get(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let company = company_from(&headers);
    let mut page = journal_page(
        &state,
        company.clone(),
        JournalFields::default(),
        String::new(),
        String::new(),
    );
    if !company.is_empty() {
        match authorize_company(&state, &company).await {
            Ok(path) => match load_postings(&path, 15).await {
                Ok(rows) => page.postings = rows,
                Err(e) => {
                    page.has_flash_err = true;
                    page.flash_err = e;
                }
            },
            Err(e) => {
                page.has_flash_err = true;
                page.flash_err = e;
            }
        }
    }
    html_ok(page)
}
