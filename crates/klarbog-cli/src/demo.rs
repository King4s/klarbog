//! Agent demo smoke path (slice 8 + 28 + wave11) — temp company, no network.

use klarbog_core::init_company;
use klarbog_plugin_bank::{
    draft_entries_from_rows, parse_bank_csv, parse_revolut_csv, BankImportConfig,
};
use klarbog_plugin_crm::CrmPlugin;
use klarbog_plugin_invoice::{
    journal_suggestion, mark_part_paid_preview, patch_status, InvoiceConfig, InvoiceKind,
    InvoicePlugin, InvoiceStatus, NewLine,
};
use klarbog_plugin_retention::{load_retention, write_gdpr_export};
use klarbog_plugin_rules_dk::moms_post_suggestion;
use klarbog_types::Actor;
use serde_json::{json, Value};

const DK_BANK_FIXTURE: &str =
    include_str!("../../klarbog-plugin-bank/tests/fixtures/danish_bank.csv");
const REVOLUT_FIXTURE: &str =
    include_str!("../../klarbog-plugin-bank/tests/fixtures/revolut_statement.csv");

pub async fn run_agent_demo() -> anyhow::Result<Value> {
    let dir = tempfile::tempdir()?;
    let company_path = dir.path().join("demo-co");
    let actor = Actor::agent("demo");
    init_company(&company_path, "Agent Demo ApS", &actor).await?;

    let crm = CrmPlugin;
    let party = crm.upsert(&company_path, "Demo Customer ApS", None)?;

    let invoice = InvoicePlugin.create(
        &company_path,
        party.id.clone(),
        InvoiceKind::Sale,
        vec![NewLine {
            description: "Agent demo line".into(),
            amount_minor: 10_000,
            currency: "DKK".into(),
        }],
    )?;
    let suggestion = journal_suggestion(&invoice, &actor, &InvoiceConfig::default())?;
    suggestion.validate()?;

    let cfg = BankImportConfig::default();
    let dk_rows = parse_bank_csv(DK_BANK_FIXTURE)?;
    let dk_drafts = draft_entries_from_rows(&dk_rows, &cfg, &actor)?;

    let revolut_rows = parse_revolut_csv(REVOLUT_FIXTURE, Some(&cfg.currency))?;
    let revolut_drafts = draft_entries_from_rows(&revolut_rows, &cfg, &actor)?;

    patch_status(&company_path, &invoice.id, InvoiceStatus::Sent)?;
    let part_amount = 4_000;
    let (part_paid, part_entry) = mark_part_paid_preview(
        &company_path,
        &invoice.id,
        part_amount,
        &actor,
        &InvoiceConfig::default(),
    )?;
    part_entry.validate()?;

    let gdpr = write_gdpr_export(&company_path)?;
    let retention = load_retention(&company_path)?;

    // Preview-only moms 25% inclusive split (i64); never posts.
    const MOMS_GROSS: i64 = 12_500;
    let moms = moms_post_suggestion(MOMS_GROSS, "demo expense #vat25 #receipt")?
        .ok_or_else(|| anyhow::anyhow!("moms-suggest expected #vat25 suggestion"))?;
    let moms_none = moms_post_suggestion(MOMS_GROSS, "demo expense #receipt")?;

    Ok(json!({
        "temp_company": company_path.to_string_lossy(),
        "party_id": party.id,
        "party_name": party.display_name,
        "invoice_id": invoice.id.to_string(),
        "invoice_total_minor": invoice.total_minor()?,
        "invoice_status": part_paid.status,
        "part_paid_amount_minor": part_amount,
        "part_paid_journal_legs": part_entry.legs.len(),
        "remaining_minor": part_paid.remaining_minor()?,
        "journal_suggestion_legs": suggestion.legs.len(),
        "bank_draft_count_generic_dk": dk_drafts.len(),
        "bank_draft_count_revolut": revolut_drafts.len(),
        "gdpr_parties": gdpr.parties.len(),
        "gdpr_invoices": gdpr.invoices.len(),
        "retention_retain_days": retention.retain_days,
        "moms_suggest_gross_minor": moms.gross_minor,
        "moms_suggest_net_minor": moms.net_minor,
        "moms_suggest_vat_minor": moms.vat_minor,
        "moms_suggest_legs": moms.legs.len(),
        "moms_suggest_auto_post": moms.auto_post,
        "moms_suggest_optional_none": moms_none.is_none(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn agent_demo_smoke() {
        let data = run_agent_demo().await.expect("demo");
        assert_eq!(data["journal_suggestion_legs"], 2);
        assert_eq!(data["bank_draft_count_generic_dk"], 3);
        assert_eq!(data["bank_draft_count_revolut"], 3);
        assert_eq!(data["invoice_status"], "part_paid");
        assert_eq!(data["part_paid_amount_minor"], 4_000);
        assert_eq!(data["part_paid_journal_legs"], 2);
        assert_eq!(data["remaining_minor"], 6_000);
        assert_eq!(data["gdpr_parties"], 1);
        assert_eq!(data["gdpr_invoices"], 1);
        assert!(data["retention_retain_days"].as_i64().unwrap() > 0);
        assert_eq!(data["moms_suggest_gross_minor"], 12_500);
        assert_eq!(data["moms_suggest_net_minor"], 10_000);
        assert_eq!(data["moms_suggest_vat_minor"], 2_500);
        assert_eq!(data["moms_suggest_legs"], 2);
        assert_eq!(data["moms_suggest_auto_post"], false);
        assert_eq!(data["moms_suggest_optional_none"], true);
    }
}
