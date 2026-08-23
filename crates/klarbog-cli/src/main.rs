mod demo;

use chrono::Utc;
use clap::{Parser, Subcommand};
use klarbog_core::{init_company, open_existing};
use klarbog_journal::{Direction, JournalEntry, Leg};
use klarbog_plugin_retention::{
    build_retention_status_report, erase_party, load_retention, run_retention_purge,
    write_backup_manifest, write_gdpr_export, BackupManifest, ErasePartyOptions, ErasePartyReport,
    GdprExport, PurgeOptions, PurgeReport, RetentionPolicy, RetentionStatusReport,
};
use klarbog_types::{Actor, Currency, Envelope, MinorAmount, PartyId};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "klarbog", version, about = "Klarbog DEV ledger CLI")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Initialize a company directory
    Init {
        #[arg(long)]
        company: PathBuf,
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "owner")]
        actor: String,
    },
    /// Post a balanced demo expense (smoke)
    SmokePost {
        #[arg(long)]
        company: PathBuf,
        #[arg(long, default_value = "owner")]
        actor: String,
        #[arg(long, default_value_t = 100)]
        minor: i64,
    },
    Health,
    /// End-to-end agent smoke (temp company, CRM, invoice, bank fixtures)
    Demo,
    /// Regelregister: håndhævede DK-regler med kilde, §, bevis-tests og
    /// åbne huller (repo-statisk; spejler originalens `reg`)
    Reg,
    /// Write backup manifest under company `backups/<ts>/manifest.json`
    Backup {
        #[arg(long)]
        company: PathBuf,
    },
    /// Show retention policy from `retention.json`
    Retention {
        #[arg(long)]
        company: PathBuf,
    },
    /// Retention deadline status report (documents, journal, bank) as of a date
    RetentionStatus {
        #[arg(long)]
        company: PathBuf,
        #[arg(long)]
        as_of: Option<String>,
    },
    /// Write company-scoped GDPR export v1 metadata (`gdpr_export.json`)
    GdprExport {
        #[arg(long)]
        company: PathBuf,
    },
    /// Preview or apply GDPR party erasure (anonymize display_name; strip or delete docs)
    GdprEraseParty {
        #[arg(long)]
        company: PathBuf,
        #[arg(long)]
        party_id: String,
        /// Apply erasure; default is dry-run preview only
        #[arg(long)]
        confirm: bool,
        /// Delete documents referencing the party (object delete); else strip party_id
        #[arg(long)]
        delete_documents: bool,
    },
    /// Preview or apply retention purge (closed exceptions; optional orphan doc GC)
    Purge {
        #[arg(long)]
        company: PathBuf,
        /// Apply purge; default is dry-run preview only
        #[arg(long)]
        confirm: bool,
        /// Remove document metadata whose party/invoice no longer exists
        #[arg(long)]
        gc_orphan_documents: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Health => {
            println!(
                "{}",
                serde_json::to_string(&Envelope::ok(serde_json::json!({
                    "service": "klarbog-cli",
                    "version": env!("CARGO_PKG_VERSION")
                })))?
            );
        }
        Cmd::Reg => {
            let rules: Vec<_> = klarbog_plugin_rules_dk::registered_rules()
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "rule_id": r.rule_id,
                        "name": r.name,
                        "source_id": r.source_id,
                        "provisions": r.provisions,
                        "severity": r.severity,
                        "enforced_by": r.enforced_by,
                        "proven_by": r.proven_by,
                        "gaps": r.gaps,
                    })
                })
                .collect();
            let with_gaps = rules
                .iter()
                .filter(|r| !r["gaps"].as_array().unwrap().is_empty())
                .count();
            println!(
                "{}",
                serde_json::to_string(&Envelope::ok(serde_json::json!({
                    "basis": klarbog_plugin_rules_dk::REGISTRY_BASIS,
                    "enforced_rules": rules.len(),
                    "rules_with_gaps": with_gaps,
                    "rules": rules,
                })))?
            );
        }
        Cmd::Init {
            company,
            name,
            actor,
        } => {
            let actor = Actor::user(actor);
            let c = init_company(&company, &name, &actor).await?;
            println!(
                "{}",
                serde_json::to_string(&Envelope::ok(serde_json::json!({
                    "path": c.path,
                    "name": c.policy.name,
                    "actors": c.policy.actors,
                })))?
            );
        }
        Cmd::SmokePost {
            company,
            actor,
            minor,
        } => {
            let actor = Actor::user(actor);
            let c = open_existing(&company).await?;
            let amount = MinorAmount::from_minor(minor);
            let currency = Currency::new("DKK")?;
            let entry = JournalEntry {
                as_of: Utc::now(),
                memo: "smoke expense".into(),
                actor,
                legs: vec![
                    Leg {
                        account: "3000".into(),
                        direction: Direction::Debit,
                        amount,
                        currency: currency.clone(),
                        party_id: None,
                    },
                    Leg {
                        account: "2000".into(),
                        direction: Direction::Credit,
                        amount,
                        currency,
                        party_id: None,
                    },
                ],
            };
            let posted = c.post(entry).await?;
            println!(
                "{}",
                serde_json::to_string(&Envelope::ok(serde_json::json!({
                    "id": posted.id,
                    "digest": posted.digest,
                })))?
            );
        }
        Cmd::Demo => {
            let data = demo::run_agent_demo().await?;
            println!("{}", serde_json::to_string(&Envelope::ok(data))?);
        }
        Cmd::Backup { company } => {
            open_existing(&company).await?;
            let manifest: BackupManifest = write_backup_manifest(&company).await?;
            println!("{}", serde_json::to_string(&Envelope::ok(manifest))?);
        }
        Cmd::Retention { company } => {
            open_existing(&company).await?;
            let policy: RetentionPolicy = load_retention(&company)?;
            println!("{}", serde_json::to_string(&Envelope::ok(policy))?);
        }
        Cmd::RetentionStatus { company, as_of } => {
            open_existing(&company).await?;
            let as_of_date = match as_of {
                Some(text) => chrono::NaiveDate::parse_from_str(&text, "%Y-%m-%d")
                    .map_err(|_| anyhow::anyhow!("asOf must be YYYY-MM-DD"))?,
                None => Utc::now().date_naive(),
            };
            let report: RetentionStatusReport =
                build_retention_status_report(&company, as_of_date).await?;
            println!("{}", serde_json::to_string(&Envelope::ok(report))?);
        }
        Cmd::GdprExport { company } => {
            open_existing(&company).await?;
            let export: GdprExport = write_gdpr_export(&company)?;
            println!("{}", serde_json::to_string(&Envelope::ok(export))?);
        }
        Cmd::GdprEraseParty {
            company,
            party_id,
            confirm,
            delete_documents,
        } => {
            open_existing(&company).await?;
            let report: ErasePartyReport = erase_party(
                &company,
                &PartyId::new(party_id),
                ErasePartyOptions {
                    confirm,
                    delete_documents,
                },
            )
            .await?;
            println!("{}", serde_json::to_string(&Envelope::ok(report))?);
        }
        Cmd::Purge {
            company,
            confirm,
            gc_orphan_documents,
        } => {
            open_existing(&company).await?;
            let report: PurgeReport = run_retention_purge(
                &company,
                PurgeOptions {
                    confirm,
                    gc_orphan_documents,
                },
            )
            .await?;
            println!("{}", serde_json::to_string(&Envelope::ok(report))?);
        }
    }
    Ok(())
}
