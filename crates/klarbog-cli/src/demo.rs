//! Agent demo smoke path (slice 8) — temp company, no network.

use klarbog_core::init_company;
use klarbog_plugin_bank::{
    draft_entries_from_rows, parse_bank_csv, parse_revolut_csv, BankImportConfig,
};
use klarbog_plugin_crm::CrmPlugin;
use klarbog_plugin_invoice::{
    journal_suggestion, InvoiceConfig, InvoiceKind, InvoicePlugin, NewLine,
};
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

    let revolut_count = match parse_revolut_csv(REVOLUT_FIXTURE, Some(&cfg.currency)) {
        Ok(rows) => draft_entries_from_rows(&rows, &cfg, &actor)
            .map(|d| d.len())
            .unwrap_or(0),
        Err(_) => {
            // TODO: Revolut profile unavailable in this build — skip count
            0
        }
    };

    Ok(json!({
        "temp_company": company_path.to_string_lossy(),
        "party_id": party.id,
        "party_name": party.display_name,
        "invoice_id": invoice.id.to_string(),
        "invoice_total_minor": invoice.total_minor()?,
        "journal_suggestion_legs": suggestion.legs.len(),
        "bank_draft_count_generic_dk": dk_drafts.len(),
        "bank_draft_count_revolut": revolut_count,
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
    }
}
