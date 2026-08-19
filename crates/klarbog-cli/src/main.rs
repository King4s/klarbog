use chrono::Utc;
use clap::{Parser, Subcommand};
use klarbog_core::{init_company, open_existing};
use klarbog_journal::{Direction, JournalEntry, Leg};
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
    }
    Ok(())
}
