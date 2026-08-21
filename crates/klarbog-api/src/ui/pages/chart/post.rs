//! Momsafregning POST actions (two-phase, digest-bound like the journal).

use axum::extract::{Form, State};
use axum::http::HeaderMap;
use axum::response::Response;
use chrono::Utc;
use klarbog_core::{journal_commit, journal_preview};
use klarbog_journal::{Direction, JournalEntry, Leg};
use klarbog_types::{Actor, Currency, MinorAmount};
use serde::Deserialize;

use super::super::common::{authorize_company, company_from, html_ok, ACTOR};
use super::view::{chart_page, load_ledger, MomsPosition};
use super::{KOEBSMOMS, MOMS_AFREGNING, SALGSMOMS};
use crate::AppState;

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct ChartActionForm {
    pub action: String,
    pub confirm_token: String,
    pub entry_json: String,
}

/// Settlement entry: zero salgsmoms/købsmoms and move the net onto
/// Momsafregning 4500 (credit = owe SKAT, debit = receivable).
fn build_settle_entry(moms: &MomsPosition) -> Result<JournalEntry, String> {
    if !moms.has_anything() {
        return Err("Ingen moms at afregne.".into());
    }
    if moms.salgs_minor < 0 || moms.koebs_minor < 0 {
        return Err("Momssaldi står omvendt — kontroller posteringerne før afregning.".into());
    }
    let currency = Currency::new("DKK").map_err(|e| e.to_string())?;
    let mut legs = Vec::new();
    let mut leg = |account: &str, direction: Direction, minor: i64| {
        legs.push(Leg {
            account: account.to_string(),
            direction,
            amount: MinorAmount::from_minor(minor),
            currency: currency.clone(),
            party_id: None,
        });
    };
    if moms.salgs_minor > 0 {
        leg(SALGSMOMS, Direction::Debit, moms.salgs_minor);
    }
    if moms.koebs_minor > 0 {
        leg(KOEBSMOMS, Direction::Credit, moms.koebs_minor);
    }
    let net = moms.net_due_minor();
    if net > 0 {
        leg(MOMS_AFREGNING, Direction::Credit, net);
    } else if net < 0 {
        leg(MOMS_AFREGNING, Direction::Debit, -net);
    }
    Ok(JournalEntry {
        as_of: Utc::now(),
        memo: format!("momsafregning pr. {}", Utc::now().format("%Y-%m-%d")),
        actor: Actor::user(ACTOR),
        legs,
    })
}

pub async fn chart_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<ChartActionForm>,
) -> Response {
    let company = company_from(&headers);
    let path = match authorize_company(&state, &company).await {
        Ok(p) => p,
        Err(e) => return html_ok(chart_page(&state, company, e)),
    };
    let actor = Actor::user(ACTOR);
    let mut page = chart_page(&state, company.clone(), String::new());
    let moms = match load_ledger(&mut page, &path).await {
        Ok(m) => m,
        Err(e) => {
            page.has_flash_err = true;
            page.flash_err = e;
            return html_ok(page);
        }
    };
    match form.action.trim() {
        "settle_preview" => {
            let entry = match build_settle_entry(&moms) {
                Ok(e) => e,
                Err(e) => {
                    page.has_flash_err = true;
                    page.flash_err = e;
                    return html_ok(page);
                }
            };
            match journal_preview(
                &state.allowlist_root,
                std::path::Path::new(&company),
                &entry,
                &actor,
                &state.confirm,
                &state.registry,
            )
            .await
            {
                Ok(p) => {
                    page.has_preview = true;
                    page.confirm_token = p.confirm_token.token.clone();
                    page.expires_unix_ms = p.confirm_token.expires_unix_ms.to_string();
                    page.payload_digest = p.payload_digest;
                    match serde_json::to_string(&entry) {
                        Ok(j) => page.entry_json = j,
                        Err(e) => {
                            page.has_preview = false;
                            page.has_flash_err = true;
                            page.flash_err = e.to_string();
                            return html_ok(page);
                        }
                    }
                    page.has_flash_ok = true;
                    page.flash_ok = format!("Preview ok · token {}", p.confirm_token.token);
                }
                Err(e) => {
                    page.has_flash_err = true;
                    page.flash_err = e.to_string();
                }
            }
            html_ok(page)
        }
        "settle_commit" => {
            let token = form.confirm_token.trim();
            let entry: Result<JournalEntry, String> = serde_json::from_str(form.entry_json.trim())
                .map_err(|e| format!("Ugyldig entry_json ({e}) — kør preview igen."));
            match entry {
                Ok(entry) if !token.is_empty() => {
                    match journal_commit(
                        &state.allowlist_root,
                        std::path::Path::new(&company),
                        entry,
                        &actor,
                        token,
                        &state.confirm,
                        &state.registry,
                    )
                    .await
                    {
                        Ok(r) => {
                            // Re-load so saldi/moms panel reflect the settlement.
                            if let Err(e) = load_ledger(&mut page, &path).await {
                                page.flash_err = e;
                                page.has_flash_err = true;
                            }
                            page.has_flash_ok = true;
                            page.flash_ok = format!("Momsafregning bogført · {}", r.posted.id);
                        }
                        Err(e) => {
                            page.has_flash_err = true;
                            page.flash_err = e.to_string();
                        }
                    }
                    html_ok(page)
                }
                Ok(_) => {
                    page.has_flash_err = true;
                    page.flash_err = "confirm_token kræves — kør preview først.".into();
                    html_ok(page)
                }
                Err(e) => {
                    page.has_flash_err = true;
                    page.flash_err = e;
                    html_ok(page)
                }
            }
        }
        other => {
            page.has_flash_err = true;
            page.flash_err = format!("Ukendt handling: {other}");
            html_ok(page)
        }
    }
}
