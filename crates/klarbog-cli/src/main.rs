mod demo;

use chrono::Utc;
use clap::{Parser, Subcommand};
use klarbog_core::{init_company, open_existing};
use klarbog_journal::{Direction, JournalEntry, Leg};
use klarbog_plugin_retention::{
    load_retention, run_retention_purge, write_backup_manifest, write_gdpr_export, BackupManifest,
    GdprExport, PurgeOptions, PurgeReport, RetentionPolicy,
};
use klarbog_types::{Actor, Currency, Envelope, MinorAmount};
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
    /// Write GDPR export stub (`gdpr_export.json`)
    GdprExport {
        #[arg(long)]
        company: PathBuf,
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
                        account: "6000".into(),
                        direction: Direction::Debit,
                        amount,
                        currency: currency.clone(),
                        party_id: None,
                    },
                    Leg {
                        account: "5800".into(),
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
        Cmd::GdprExport { company } => {
            open_existing(&company).await?;
            let export: GdprExport = write_gdpr_export(&company)?;
            println!("{}", serde_json::to_string(&Envelope::ok(export))?);
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
