//! MCP tool registry and dispatch.

mod auth;
mod bank;
mod bank_stripe_pipelines;
mod bank_wave2;
mod helpers;
mod invoice;
mod journal;
mod moms_suggest;
mod registry;
mod retention;

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
        "bank_import_preview" => rt.block_on(bank::bank_import_preview(args, allowlist_root)),
        "bank_stripe_consume" => rt.block_on(bank_wave2::bank_stripe_consume(args, allowlist_root)),
        "bank_reconcile_apply" => {
            rt.block_on(bank_wave2::bank_reconcile_apply(args, allowlist_root))
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
        "gdpr_erase_party" => rt.block_on(retention::gdpr_erase_party(args, allowlist_root)),
        "invoice_mark_paid_preview" => {
            rt.block_on(invoice::invoice_mark_paid_preview(args, allowlist_root))
        }
        "invoice_mark_part_paid_preview" => rt.block_on(invoice::invoice_mark_part_paid_preview(
            args,
            allowlist_root,
        )),
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
        other => Envelope::err([format!("unknown tool: {other}")]),
    }
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
