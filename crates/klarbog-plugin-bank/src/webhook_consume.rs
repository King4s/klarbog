//! Stripe webhook queue consumer (ADR-009). Emits BankRow drafts — no journal post.

use crate::csv::BankRow;
use crate::webhook::{
    draft_to_bank_row, QueuedStripeWebhook, StripeWebhookError, STRIPE_WEBHOOKS_DIR,
    STRIPE_WEBHOOKS_QUEUE,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

/// Sidecar of consumed event ids (jsonl). Idempotency by Stripe event id.
pub const STRIPE_WEBHOOKS_CONSUMED: &str = "stripe_webhooks/queue.consumed";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsumeOpts {
    /// When true (default), preview only — do not append to `queue.consumed`.
    pub dry_run: bool,
    /// Max new events to take this pass (already-consumed always skipped).
    pub limit: Option<usize>,
}

impl Default for ConsumeOpts {
    fn default() -> Self {
        Self {
            dry_run: true,
            limit: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsumedBankRow {
    pub event_id: String,
    pub event_type: String,
    pub object_id: String,
    pub date: chrono::DateTime<chrono::Utc>,
    pub text: String,
    pub amount_minor: i64,
    pub currency: String,
}

impl ConsumedBankRow {
    pub fn to_bank_row(&self) -> BankRow {
        BankRow {
            date: self.date,
            text: self.text.clone(),
            amount_minor: klarbog_types::MinorAmount::from_minor(self.amount_minor),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsumeReport {
    pub dry_run: bool,
    pub consumed_count: usize,
    pub skipped_already: usize,
    pub skipped_no_draft: usize,
    pub rows: Vec<ConsumedBankRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConsumedSidecarLine {
    event_id: String,
    consumed_unix_ms: i64,
}

fn load_consumed_ids(company: &Path) -> Result<HashSet<String>, StripeWebhookError> {
    let path = company.join(STRIPE_WEBHOOKS_CONSUMED);
    if !path.exists() {
        return Ok(HashSet::new());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| StripeWebhookError::Io(e.to_string()))?;
    let mut ids = HashSet::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let entry: ConsumedSidecarLine = serde_json::from_str(line).map_err(|e| {
            StripeWebhookError::InvalidJson(format!("queue.consumed line {}: {e}", i + 1))
        })?;
        ids.insert(entry.event_id);
    }
    Ok(ids)
}

fn append_consumed(company: &Path, event_ids: &[String]) -> Result<(), StripeWebhookError> {
    if event_ids.is_empty() {
        return Ok(());
    }
    let dir = company.join(STRIPE_WEBHOOKS_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| StripeWebhookError::Io(e.to_string()))?;
    let path = company.join(STRIPE_WEBHOOKS_CONSUMED);
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| StripeWebhookError::Io(e.to_string()))?;
    let now = chrono::Utc::now().timestamp_millis();
    use std::io::Write;
    for event_id in event_ids {
        let line = serde_json::to_string(&ConsumedSidecarLine {
            event_id: event_id.clone(),
            consumed_unix_ms: now,
        })
        .map_err(|e| StripeWebhookError::InvalidJson(e.to_string()))?;
        writeln!(file, "{line}").map_err(|e| StripeWebhookError::Io(e.to_string()))?;
    }
    Ok(())
}

fn read_queue(company: &Path) -> Result<Vec<QueuedStripeWebhook>, StripeWebhookError> {
    let path = company.join(STRIPE_WEBHOOKS_QUEUE);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| StripeWebhookError::Io(e.to_string()))?;
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let entry: QueuedStripeWebhook = serde_json::from_str(line).map_err(|e| {
            StripeWebhookError::InvalidJson(format!("queue.jsonl line {}: {e}", i + 1))
        })?;
        out.push(entry);
    }
    Ok(out)
}

/// Read `{company}/stripe_webhooks/queue.jsonl`, skip already-consumed event ids,
/// emit bank-compatible draft rows. Persist ids to `queue.consumed` unless `dry_run`.
pub fn consume_stripe_webhook_queue(
    company: &Path,
    opts: ConsumeOpts,
) -> Result<ConsumeReport, StripeWebhookError> {
    let already = load_consumed_ids(company)?;
    let queue = read_queue(company)?;
    let mut skipped_already = 0usize;
    let mut skipped_no_draft = 0usize;
    let mut rows = Vec::new();
    let mut newly_consumed = Vec::new();

    for entry in queue {
        if already.contains(&entry.event_id) {
            skipped_already += 1;
            continue;
        }
        if newly_consumed.iter().any(|id| id == &entry.event_id) {
            // Same event id queued twice in one file — consume once.
            skipped_already += 1;
            continue;
        }
        let Some(draft) = entry.draft.as_ref() else {
            skipped_no_draft += 1;
            newly_consumed.push(entry.event_id.clone());
            continue;
        };
        if let Some(limit) = opts.limit {
            if rows.len() >= limit {
                break;
            }
        }
        let bank = draft_to_bank_row(draft);
        rows.push(ConsumedBankRow {
            event_id: entry.event_id.clone(),
            event_type: entry.event_type.clone(),
            object_id: entry.object_id.clone(),
            date: bank.date,
            text: bank.text,
            amount_minor: bank.amount_minor.minor(),
            currency: draft.currency.clone(),
        });
        newly_consumed.push(entry.event_id);
    }

    if !opts.dry_run {
        append_consumed(company, &newly_consumed)?;
    }

    Ok(ConsumeReport {
        dry_run: opts.dry_run,
        consumed_count: rows.len(),
        skipped_already,
        skipped_no_draft,
        rows,
    })
}
