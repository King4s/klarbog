//! One-shot Stripe consume → reconcile suggest (ADR-009). No journal post.

use crate::reconcile::{
    list_unmatched_bank_exceptions, suggest_matches, sync_unmatched_exceptions, BankRowMatchResult,
    ReconcileError,
};
use crate::webhook::StripeWebhookError;
use crate::webhook_consume::{consume_stripe_webhook_queue, ConsumeOpts, ConsumeReport};
use klarbog_plugin_documents::Exception;
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StripeReconcilePipelineError {
    #[error(transparent)]
    Webhook(#[from] StripeWebhookError),
    #[error(transparent)]
    Reconcile(#[from] ReconcileError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StripeReconcileSuggestReport {
    pub consume: ConsumeReport,
    pub matches: Vec<BankRowMatchResult>,
    pub exceptions_raised: Vec<String>,
    pub exceptions: Vec<Exception>,
}

/// Consume Stripe webhook queue rows, then run reconcile suggest on those drafts.
/// Still **no** journal post — apply/confirm remains a separate step.
pub fn suggest_from_stripe_consume(
    company: &Path,
    opts: ConsumeOpts,
) -> Result<StripeReconcileSuggestReport, StripeReconcilePipelineError> {
    suggest_from_stripe_consume_with(company, opts, true)
}

/// Same as [`suggest_from_stripe_consume`] with optional exception sync.
pub fn suggest_from_stripe_consume_with(
    company: &Path,
    opts: ConsumeOpts,
    raise_exceptions: bool,
) -> Result<StripeReconcileSuggestReport, StripeReconcilePipelineError> {
    let consume = consume_stripe_webhook_queue(company, opts)?;
    let bank_rows: Vec<_> = consume.rows.iter().map(|r| r.to_bank_row()).collect();
    let matches = suggest_matches(company, &bank_rows)?;
    let raised = if raise_exceptions {
        sync_unmatched_exceptions(company, &bank_rows, &matches)?
    } else {
        Vec::new()
    };
    let open = list_unmatched_bank_exceptions(company)?;
    Ok(StripeReconcileSuggestReport {
        consume,
        matches,
        exceptions_raised: raised.iter().map(|e| e.id.to_string()).collect(),
        exceptions: open,
    })
}
