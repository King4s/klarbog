//! Journal POST actions (moms / preview / commit).

use axum::extract::{Form, State};
use axum::http::HeaderMap;
use axum::response::Response;
use klarbog_core::{journal_commit, journal_preview};
use klarbog_plugin_rules_dk::moms_post_suggestion;
use klarbog_types::Actor;

use super::super::common::{authorize_company, company_from, format_dkk, html_ok, ACTOR};
use super::form::{
    build_entry, build_moms_split_entry, fields_from_form, JournalActionForm, JournalFields,
};
use super::view::journal_page;
use crate::AppState;

/// Load a posted entry by id from the authorized company (fail-closed on miss).
async fn load_posted(
    state: &AppState,
    company: &str,
    id: &str,
) -> Result<klarbog_journal::PostedEntry, String> {
    use klarbog_core::assert_company_path;
    let path = assert_company_path(&state.allowlist_root, std::path::Path::new(company))
        .map_err(|e| e.to_string())?;
    let c = klarbog_core::open_existing(&path)
        .await
        .map_err(|e| e.to_string())?;
    c.posted_entry(id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Postering {id} findes ikke."))
}

/// Journal-preview `entry` and render the confirm panel; entry_json carries the
/// exact previewed entry (digest-bound token includes as_of — never rebuild).
async fn render_preview(
    state: &AppState,
    company: String,
    actor: &Actor,
    entry: klarbog_journal::JournalEntry,
    mut fields: JournalFields,
) -> Response {
    match journal_preview(
        &state.allowlist_root,
        std::path::Path::new(&company),
        &entry,
        actor,
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
            fields.entry_json = match serde_json::to_string(&entry) {
                Ok(j) => j,
                Err(e) => {
                    return html_ok(journal_page(
                        state,
                        company,
                        JournalFields::default(),
                        String::new(),
                        e.to_string(),
                    ));
                }
            };
            html_ok(journal_page(
                state,
                company,
                fields,
                format!("Preview ok · token {}", p.confirm_token.token),
                String::new(),
            ))
        }
        Err(e) => html_ok(journal_page(
            state,
            company,
            fields,
            String::new(),
            e.to_string(),
        )),
    }
}

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
        // Book the suggestion as a real 3-leg VAT split (net → expense,
        // vat → Købsmoms 4000, gross → credit) through preview→commit.
        "moms_apply" => {
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
            let suggestion = match moms_post_suggestion(gross, &form.moms_memo) {
                Ok(Some(s)) => s,
                Ok(None) => {
                    return html_ok(journal_page(
                        &state,
                        company,
                        fields,
                        String::new(),
                        "Memo mangler momstag (#vat25 / moms:25 / #moms25).".into(),
                    ));
                }
                Err(e) => {
                    return html_ok(journal_page(
                        &state,
                        company,
                        fields,
                        String::new(),
                        e.to_string(),
                    ));
                }
            };
            let entry = match build_moms_split_entry(
                &form.moms_memo,
                suggestion.net_minor,
                suggestion.vat_minor,
                suggestion.gross_minor,
                &fields.account1,
                &fields.account2,
            ) {
                Ok(e) => e,
                Err(e) => return html_ok(journal_page(&state, company, fields, String::new(), e)),
            };
            fields.memo = form.moms_memo.clone();
            fields.amount1 = suggestion.net_minor.to_string();
            fields.amount2 = suggestion.gross_minor.to_string();
            fields.has_moms = true;
            fields.moms_suggested = true;
            fields.moms_gross_minor = suggestion.gross_minor.to_string();
            fields.moms_net_minor = suggestion.net_minor.to_string();
            fields.moms_vat_minor = suggestion.vat_minor.to_string();
            fields.moms_rate_bps = suggestion.rate_bps.to_string();
            render_preview(&state, company, &actor, entry, fields).await
        }
        "preview" => match build_entry(&form) {
            Ok(entry) => render_preview(&state, company, &actor, entry, fields).await,
            Err(e) => html_ok(journal_page(&state, company, fields, String::new(), e)),
        },
        "reverse_preview" => {
            let id = form.entry_id.trim();
            if id.is_empty() {
                return html_ok(journal_page(
                    &state,
                    company,
                    fields,
                    String::new(),
                    "entry_id kræves til tilbageførsel.".into(),
                ));
            }
            let original = match load_posted(&state, &company, id).await {
                Ok(p) => p,
                Err(e) => return html_ok(journal_page(&state, company, fields, String::new(), e)),
            };
            let entry = original.entry.reversal(
                chrono::Utc::now(),
                actor.clone(),
                format!("tilbageførsel af {id} · {}", original.entry.memo),
            );
            fields = JournalFields {
                memo: entry.memo.clone(),
                ..Default::default()
            };
            render_preview(&state, company, &actor, entry, fields).await
        }
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
