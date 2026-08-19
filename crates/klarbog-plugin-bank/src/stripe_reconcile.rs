//! One-shot Stripe consume → reconcile suggest / apply-preview (ADR-009).
//! Never posts the journal — ConfirmStore commit stays with the host.

use crate::reconcile::{
    list_unmatched_bank_exceptions, suggest_matches, sync_unmatched_exceptions, BankRowMatchResult,
    ReconcileError, SAFE_THRESHOLD_BPS,
};
use crate::reconcile_apply::{apply_match, ApplyMatchResult};
use crate::webhook::StripeWebhookError;
use crate::webhook_consume::{consume_stripe_webhook_queue, ConsumeOpts, ConsumeReport};
use klarbog_plugin_documents::Exception;
use klarbog_types::Actor;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripeReconcileApplied {
    pub row_index: usize,
    pub invoice_id: String,
    #[serde(flatten)]
    pub result: ApplyMatchResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripeReconcileApplyPreviewReport {
    pub consume: ConsumeReport,
    pub matches: Vec<BankRowMatchResult>,
    pub exceptions_raised: Vec<String>,
    pub exceptions: Vec<Exception>,
    /// Present only when exactly one best suggestion is ≥ [`SAFE_THRESHOLD_BPS`].
    pub applied: Option<StripeReconcileApplied>,
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

/// Consume → suggest → optionally `apply_match` when exactly one safe best suggestion.
/// Does **not** run ConfirmStore / journal commit (host wires `preview:true`).
pub fn apply_preview_from_stripe_consume(
    company: &Path,
    opts: ConsumeOpts,
    actor: &Actor,
    force: bool,
) -> Result<StripeReconcileApplyPreviewReport, StripeReconcilePipelineError> {
    apply_preview_from_stripe_consume_with(company, opts, actor, force, true)
}

/// Same as [`apply_preview_from_stripe_consume`] with optional exception sync.
pub fn apply_preview_from_stripe_consume_with(
    company: &Path,
    opts: ConsumeOpts,
    actor: &Actor,
    force: bool,
    raise_exceptions: bool,
) -> Result<StripeReconcileApplyPreviewReport, StripeReconcilePipelineError> {
    let suggest = suggest_from_stripe_consume_with(company, opts, raise_exceptions)?;
    let bank_rows: Vec<_> = suggest
        .consume
        .rows
        .iter()
        .map(|r| r.to_bank_row())
        .collect();
    let applied = try_apply_unique_safe(&bank_rows, &suggest.matches, company, actor, force)?;
    let open = list_unmatched_bank_exceptions(company)?;
    Ok(StripeReconcileApplyPreviewReport {
        consume: suggest.consume,
        matches: suggest.matches,
        exceptions_raised: suggest.exceptions_raised,
        exceptions: open,
        applied,
    })
}

fn try_apply_unique_safe(
    bank_rows: &[crate::csv::BankRow],
    matches: &[BankRowMatchResult],
    company: &Path,
    actor: &Actor,
    force: bool,
) -> Result<Option<StripeReconcileApplied>, StripeReconcilePipelineError> {
    let safe: Vec<_> = matches
        .iter()
        .filter_map(|m| {
            let s = m.suggestions.first()?;
            (s.confidence_bps >= SAFE_THRESHOLD_BPS).then_some((m.row_index, s.invoice_id.clone()))
        })
        .collect();
    if safe.len() != 1 {
        return Ok(None);
    }
    let (row_index, invoice_id) = &safe[0];
    let row = bank_rows
        .get(*row_index)
        .ok_or(StripeReconcilePipelineError::Reconcile(
            ReconcileError::AmountMismatch,
        ))?;
    let result = apply_match(company, row, invoice_id, actor, force, *row_index)?;
    Ok(Some(StripeReconcileApplied {
        row_index: *row_index,
        invoice_id: invoice_id.clone(),
        result,
    }))
}
