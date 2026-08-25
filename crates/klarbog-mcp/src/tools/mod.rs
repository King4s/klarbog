//! MCP tool registry and dispatch.

/// Serializes process-env mutation across Revolut/Stripe/OAuth MCP tests (parallel VERIFY).
#[cfg(test)]
pub(crate) static ENV_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

mod auth;
mod bank;
mod bank_oauth;
mod bank_stripe_pipelines;
mod bank_wave2;
mod crm;
mod documents;
mod email;
mod helpers;
mod invoice;
mod invoice_settlement;
mod journal;
mod moms_suggest;
mod registry;
mod retention;
mod rules_chart;

pub use helpers::default_allowlist_root;
pub use registry::tools_list;

use klarbog_core::ConfirmStore;
use klarbog_plugin::Registry;
use klarbog_types::Envelope;
use serde_json::{json, Value};
use std::path::Path;

pub fn handle_tool_call(
    name: &str,
    args: &Value,
    store: &ConfirmStore,
    allowlist_root: &Path,
    registry: &Registry,
    rt: &tokio::runtime::Runtime,
) -> Envelope<Value> {
    match name {
        "klarbog_health" => Envelope::ok(json!({
            "ok": true,
            "service": "klarbog-mcp",
            "allowlist_root": allowlist_root,
        })),
        "klarbog_status" => Envelope::ok(helpers::status_payload(allowlist_root, registry)),
        "crm_upsert_party" => rt.block_on(crm::crm_upsert_party(args, allowlist_root)),
        "crm_list_parties" => rt.block_on(crm::crm_list_parties(args, allowlist_root)),
        "documents_attach" => rt.block_on(documents::documents_attach(args, allowlist_root)),
        "documents_list" => rt.block_on(documents::documents_list(args, allowlist_root)),
        "documents_delete" => rt.block_on(documents::documents_delete(args, allowlist_root)),
        "exceptions_raise" => rt.block_on(documents::exceptions_raise(args, allowlist_root)),
        "exceptions_list" => rt.block_on(documents::exceptions_list(args, allowlist_root)),
        "exceptions_set_open" => rt.block_on(documents::exceptions_set_open(args, allowlist_root)),
        "bank_import_preview" => rt.block_on(bank::bank_import_preview(args, allowlist_root)),
        "bank_stripe_consume" => rt.block_on(bank_wave2::bank_stripe_consume(args, allowlist_root)),
        "bank_reconcile_suggest" => {
            rt.block_on(bank_wave2::bank_reconcile_suggest(args, allowlist_root))
        }
        "bank_reconcile_apply" => rt.block_on(bank_wave2::bank_reconcile_apply(
            args,
            allowlist_root,
            store,
            registry,
        )),
        "revolut_oauth_start" => rt.block_on(bank_oauth::revolut_oauth_start(args, allowlist_root)),
        "revolut_oauth_callback" => {
            rt.block_on(bank_oauth::revolut_oauth_callback(args, allowlist_root))
        }
        "revolut_oauth_refresh" => {
            rt.block_on(bank_wave2::revolut_oauth_refresh(args, allowlist_root))
        }
        "bank_stripe_reconcile_suggest" => rt.block_on(
            bank_stripe_pipelines::bank_stripe_reconcile_suggest(args, allowlist_root),
        ),
        "bank_stripe_reconcile_apply_preview" => {
            rt.block_on(bank_stripe_pipelines::bank_stripe_reconcile_apply_preview(
                args,
                allowlist_root,
                store,
                registry,
            ))
        }
        "retention_get" => rt.block_on(retention::retention_get(args, allowlist_root)),
        "retention_purge" => rt.block_on(retention::retention_purge(args, allowlist_root)),
        "backup_manifest" => rt.block_on(retention::backup_manifest(args, allowlist_root)),
        "gdpr_export" => rt.block_on(retention::gdpr_export(args, allowlist_root)),
        "gdpr_erase_party" => rt.block_on(retention::gdpr_erase_party(args, allowlist_root)),
        "invoice_create_draft" => rt.block_on(invoice::invoice_create_draft(args, allowlist_root)),
        "invoice_list" => rt.block_on(invoice::invoice_list(args, allowlist_root)),
        "invoice_patch_status" => rt.block_on(invoice::invoice_patch_status(args, allowlist_root)),
        "invoice_mark_paid_preview" => rt.block_on(invoice::invoice_mark_paid_preview(
            args,
            allowlist_root,
            store,
            registry,
        )),
        "invoice_mark_part_paid_preview" => rt.block_on(invoice::invoice_mark_part_paid_preview(
            args,
            allowlist_root,
            store,
            registry,
        )),
        "invoice_send_email" => rt.block_on(email::invoice_send_email(args, allowlist_root)),
        "invoice_compensation_calc" => rt.block_on(invoice_settlement::invoice_compensation_calc(
            args,
            allowlist_root,
        )),
        "invoice_claim_compensation" => rt.block_on(
            invoice_settlement::invoice_claim_compensation(args, allowlist_root),
        ),
        "invoice_post_compensation_preview" => {
            rt.block_on(invoice_settlement::invoice_post_compensation_preview(
                args,
                allowlist_root,
                store,
                registry,
            ))
        }
        "invoice_interest_calc" => rt.block_on(invoice_settlement::invoice_interest_calc(
            args,
            allowlist_root,
        )),
        "invoice_claim_interest" => rt.block_on(invoice_settlement::invoice_claim_interest(
            args,
            allowlist_root,
        )),
        "invoice_post_interest_preview" => {
            rt.block_on(invoice_settlement::invoice_post_interest_preview(
                args,
                allowlist_root,
                store,
                registry,
            ))
        }
        "journal_post_preview" => rt.block_on(journal::journal_post_preview(
            args,
            allowlist_root,
            store,
            registry,
        )),
        "journal_post_commit" => rt.block_on(journal::journal_post_commit(
            args,
            allowlist_root,
            store,
            registry,
        )),
        "journal_moms_post_suggestion" => rt.block_on(moms_suggest::journal_moms_post_suggestion(
            args,
            allowlist_root,
        )),
        "rules_chart_list" => rt.block_on(rules_chart::rules_chart_list(args, allowlist_root)),
        other => Envelope::err([format!("unknown tool: {other}")]),
    }
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
