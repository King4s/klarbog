//! Journal POST actions (moms / preview / commit).

use axum::extract::{Form, State};
use axum::http::HeaderMap;
use axum::response::Response;
use klarbog_core::{journal_commit, journal_preview};
use klarbog_plugin_rules_dk::moms_post_suggestion;
use klarbog_types::Actor;

use super::super::common::{authorize_company, company_from, format_dkk, html_ok, ACTOR};
use super::form::{build_entry, fields_from_form, JournalActionForm, JournalFields};
use super::view::journal_page;
use crate::AppState;

pub async fn journal_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<JournalActionForm>,
) -> Response {
    let company = company_from(&headers);
    let mut fields = fields_from_form(&form);
    if company.is_empty() {
        return html_ok(journal_page(
            &state,
            company,
            fields,
            String::new(),
            "Sæt firmasti under Indstillinger.".into(),
        ));
    }
    if let Err(e) = authorize_company(&state, &company).await {
        return html_ok(journal_page(&state, company, fields, String::new(), e));
    }
    let actor = Actor::user(ACTOR);
    match form.action.trim() {
        "moms_suggest" => {
            let gross: i64 = match form.moms_gross.trim().parse() {
                Ok(v) => v,
                Err(_) => {
                    return html_ok(journal_page(
                        &state,
                        company,
                        fields,
                        String::new(),
                        "Brutto skal være heltal (øre).".into(),
                    ));
                }
            };
            match moms_post_suggestion(gross, &form.moms_memo) {
                Ok(Some(s)) => {
                    fields.has_moms = true;
                    fields.moms_suggested = true;
                    fields.moms_gross_minor = s.gross_minor.to_string();
                    fields.moms_net_minor = s.net_minor.to_string();
                    fields.moms_vat_minor = s.vat_minor.to_string();
                    fields.moms_rate_bps = s.rate_bps.to_string();
                    html_ok(journal_page(
                        &state,
                        company,
                        fields,
                        format!(
                            "Moms-forslag: net {} · moms {} · brutto {}",
                            format_dkk(s.net_minor),
                            format_dkk(s.vat_minor),
                            format_dkk(s.gross_minor)
                        ),
                        String::new(),
                    ))
                }
                Ok(None) => {
                    fields.has_moms = true;
                    fields.moms_reason = "memo har ingen #vat25 / moms:25 / #moms25 tag".into();
                    html_ok(journal_page(
                        &state,
                        company,
                        fields,
                        String::new(),
                        String::new(),
                    ))
                }
                Err(e) => html_ok(journal_page(
                    &state,
                    company,
                    fields,
                    String::new(),
                    e.to_string(),
                )),
            }
        }
        "moms_apply" => {
            let gross: i64 = form.moms_gross.trim().parse().unwrap_or(12500);
            fields.amount1 = gross.to_string();
            fields.amount2 = gross.to_string();
            fields.memo = form.moms_memo.clone();
            html_ok(journal_page(
                &state,
                company,
                fields,
                "Anvendt brutto på begge ben.".into(),
                String::new(),
            ))
        }
        "preview" => match build_entry(&form) {
            Ok(entry) => match journal_preview(
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
                    fields.has_preview = true;
                    fields.confirm_token = p.confirm_token.token.clone();
                    fields.expires_unix_ms = p.confirm_token.expires_unix_ms.to_string();
                    fields.payload_digest = p.payload_digest;
                    // Carry the exact previewed entry to commit: rebuilding from
                    // form fields would stamp a new as_of and break the digest.
                    fields.entry_json = match serde_json::to_string(&entry) {
                        Ok(j) => j,
                        Err(e) => {
                            return html_ok(journal_page(
                                &state,
                                company,
                                fields_from_form(&form),
                                String::new(),
                                e.to_string(),
                            ));
                        }
                    };
                    html_ok(journal_page(
                        &state,
                        company,
                        fields,
                        format!("Preview ok · token {}", p.confirm_token.token),
                        String::new(),
                    ))
                }
                Err(e) => html_ok(journal_page(
                    &state,
                    company,
                    fields,
                    String::new(),
                    e.to_string(),
                )),
            },
            Err(e) => html_ok(journal_page(&state, company, fields, String::new(), e)),
        },
        "commit" => {
            let token = form.confirm_token.trim();
            if token.is_empty() {
                return html_ok(journal_page(
                    &state,
                    company,
                    fields,
                    String::new(),
                    "confirm_token kræves — kør preview først.".into(),
                ));
            }
            let entry: Result<klarbog_journal::JournalEntry, String> =
                serde_json::from_str(form.entry_json.trim())
                    .map_err(|e| format!("Ugyldig entry_json ({e}) — kør preview igen."));
            match entry {
                Ok(entry) => match journal_commit(
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
                    Ok(r) => html_ok(journal_page(
                        &state,
                        company,
                        JournalFields::default(),
                        format!("Commit ok · posted {}", r.posted.id),
                        String::new(),
                    )),
                    Err(e) => html_ok(journal_page(
                        &state,
                        company,
                        fields,
                        String::new(),
                        e.to_string(),
                    )),
                },
                Err(e) => html_ok(journal_page(&state, company, fields, String::new(), e)),
            }
        }
        _ => html_ok(journal_page(
            &state,
            company,
            fields,
            String::new(),
            format!("Ukendt handling: {}", form.action),
        )),
    }
}
