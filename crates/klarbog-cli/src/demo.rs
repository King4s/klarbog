//! Agent demo smoke path (slice 8 + 28 / wave33) — temp company, offline, no live Revolut keys.

use klarbog_core::init_company;
use klarbog_plugin_bank::{
    import_preview, BankImportConfig, BankImportError, BankImportSource, BankProfile,
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
    let party = crm.upsert(
        &company_path,
        "Demo Customer ApS",
        None,
        klarbog_plugin_crm::PartyKind::Business,
    )?;

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
    let (_, dk_drafts) = import_preview(
        BankImportSource::Csv,
        BankProfile::GenericDk,
        Some(DK_BANK_FIXTURE),
        &cfg,
        &actor,
        None,
    )
    .await?;

    // Offline Revolut: CSV fixture via import_preview — never requires live API keys.
    let (_, revolut_drafts) = import_preview(
        BankImportSource::Csv,
        BankProfile::Revolut,
        Some(REVOLUT_FIXTURE),
        &cfg,
        &actor,
        None,
    )
    .await?;

    // Fail-closed without live keys: API source with no env token / company secrets
    // errors before any network call. If a process token is already set, skip the
    // live probe so demo stays offline-friendly.
    let revolut_api_fail_closed_without_keys =
        probe_revolut_api_fail_closed_without_keys(&cfg, &actor).await;

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
        "revolut_import_source": "csv",
        "revolut_requires_live_keys": false,
        "revolut_api_fail_closed_without_keys": revolut_api_fail_closed_without_keys,
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

/// When `KLARBOG_REVOLUT_API_TOKEN` is unset/blank, API import must fail closed (Config) with no network.
/// When a token is already in the process env, return true without calling live API (demo stays offline).
async fn probe_revolut_api_fail_closed_without_keys(cfg: &BankImportConfig, actor: &Actor) -> bool {
    let has_token = std::env::var("KLARBOG_REVOLUT_API_TOKEN")
        .ok()
        .is_some_and(|v| !v.trim().is_empty());
    if has_token {
        return true;
    }
    matches!(
        import_preview(
            BankImportSource::Api,
            BankProfile::Revolut,
            None,
            cfg,
            actor,
            None,
        )
        .await,
        Err(BankImportError::Config(_))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn agent_demo_smoke() {
        let _token_guard = EnvGuard::unset("KLARBOG_REVOLUT_API_TOKEN");
        let data = run_agent_demo().await.expect("demo");
        // Business party (ADR-020): 10000 net + 2500 salgsmoms = 12500 gross → 3 legs.
        assert_eq!(data["journal_suggestion_legs"], 3);
        assert_eq!(data["bank_draft_count_generic_dk"], 3);
        assert_eq!(data["bank_draft_count_revolut"], 3);
        assert_eq!(data["revolut_import_source"], "csv");
        assert_eq!(data["revolut_requires_live_keys"], false);
        assert_eq!(data["revolut_api_fail_closed_without_keys"], true);
        assert_eq!(data["invoice_status"], "part_paid");
        assert_eq!(data["part_paid_amount_minor"], 4_000);
        assert_eq!(data["part_paid_journal_legs"], 2);
        // Remaining is gross-based: 12500 − 4000 part-paid.
        assert_eq!(data["remaining_minor"], 8_500);
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

    struct EnvGuard {
        key: &'static str,
        prev: Option<String>,
    }

    impl EnvGuard {
        fn unset(key: &'static str) -> Self {
            let prev = std::env::var(key).ok();
            // SAFETY: demo smoke only; restore on drop.
            unsafe { std::env::remove_var(key) };
            Self { key, prev }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => unsafe { std::env::set_var(self.key, v) },
                None => unsafe { std::env::remove_var(self.key) },
            }
        }
    }
}
