//! Compile-time plugins only (ADR-002). No dynamic .so loading.

use klarbog_journal::JournalEntry;
use klarbog_types::KlarbogError;

/// Capability flags. CRM plugins must not include JournalWrite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    Read,
    CrmWrite,
    JournalWrite,
    RulesValidate,
}

pub trait Plugin: Send + Sync {
    fn id(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn capabilities(&self) -> &'static [Capability];

    fn has_journal_write(&self) -> bool {
        self.capabilities().contains(&Capability::JournalWrite)
    }
}

/// Optional rules plugin — validates entries before preview/commit.
pub trait RulesPlugin: Plugin {
    fn validate_entry(&self, entry: &JournalEntry) -> Result<Vec<String>, KlarbogError>;
}

/// Host registry — explicit static registration.
pub struct Registry {
    plugins: Vec<&'static dyn Plugin>,
    rules: Vec<&'static dyn RulesPlugin>,
}

impl Registry {
    pub fn new(
        plugins: Vec<&'static dyn Plugin>,
        rules: Vec<&'static dyn RulesPlugin>,
    ) -> Result<Self, KlarbogError> {
        for p in &plugins {
            if p.has_journal_write() && p.id() == "crm" {
                return Err(KlarbogError::Message(
                    "crm plugin must not advertise JournalWrite".into(),
                ));
            }
        }
        for r in &rules {
            if !r.capabilities().contains(&Capability::RulesValidate) {
                return Err(KlarbogError::Message(format!(
                    "rules plugin {} missing RulesValidate capability",
                    r.id()
                )));
            }
            if !plugins.iter().any(|p| p.id() == r.id()) {
                return Err(KlarbogError::Message(format!(
                    "rules plugin {} not registered in plugins list",
                    r.id()
                )));
            }
        }
        Ok(Self { plugins, rules })
    }

    pub fn list(&self) -> impl Iterator<Item = &dyn Plugin> {
        self.plugins.iter().copied()
    }

    pub fn list_by_capability(&self, cap: Capability) -> impl Iterator<Item = &dyn Plugin> + '_ {
        self.plugins
            .iter()
            .copied()
            .filter(move |p| p.capabilities().contains(&cap))
    }

    pub fn rules_plugins(&self) -> impl Iterator<Item = &dyn RulesPlugin> + '_ {
        self.rules.iter().copied()
    }

    /// Run all registered rules plugins; aggregate applied rule ids.
    pub fn validate_rules(&self, entry: &JournalEntry) -> Result<Vec<String>, KlarbogError> {
        let mut applied = Vec::new();
        for plugin in self.rules_plugins() {
            let mut rules = plugin.validate_entry(entry)?;
            applied.append(&mut rules);
        }
        applied.sort();
        applied.dedup();
        Ok(applied)
    }
}

/// Example read-only meta plugin used at bootstrap.
pub struct MetaPlugin;

impl Plugin for MetaPlugin {
    fn id(&self) -> &'static str {
        "meta"
    }
    fn version(&self) -> &'static str {
        "0.1.0"
    }
    fn capabilities(&self) -> &'static [Capability] {
        &[Capability::Read]
    }
}

pub static META: MetaPlugin = MetaPlugin;

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use klarbog_journal::{Direction, Leg};
    use klarbog_types::{Actor, Currency, MinorAmount};

    struct StubRules;

    impl Plugin for StubRules {
        fn id(&self) -> &'static str {
            "stub-rules"
        }
        fn version(&self) -> &'static str {
            "0.0.1"
        }
        fn capabilities(&self) -> &'static [Capability] {
            &[Capability::RulesValidate]
        }
    }

    impl RulesPlugin for StubRules {
        fn validate_entry(&self, entry: &JournalEntry) -> Result<Vec<String>, KlarbogError> {
            if entry.memo.is_empty() {
                return Err(KlarbogError::Message("memo required".into()));
            }
            Ok(vec!["stub.ok".into()])
        }
    }

    static STUB_RULES: StubRules = StubRules;

    fn sample_entry(memo: &str) -> JournalEntry {
        let amount = MinorAmount::from_minor(100);
        let currency = Currency::new("DKK").unwrap();
        JournalEntry {
            as_of: Utc::now(),
            memo: memo.into(),
            actor: Actor::user("t"),
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
        }
    }

    #[test]
    fn meta_has_no_journal_write() {
        assert!(!META.has_journal_write());
        let reg = Registry::new(vec![&META], vec![]).unwrap();
        assert_eq!(reg.list().count(), 1);
        assert_eq!(reg.list_by_capability(Capability::Read).count(), 1);
        assert_eq!(reg.list_by_capability(Capability::RulesValidate).count(), 0);
    }

    #[test]
    fn validate_rules_runs_plugins() {
        let reg = Registry::new(vec![&META, &STUB_RULES], vec![&STUB_RULES]).unwrap();
        let applied = reg.validate_rules(&sample_entry("ok")).unwrap();
        assert_eq!(applied, vec!["stub.ok"]);
        assert!(reg.validate_rules(&sample_entry("")).is_err());
    }
}
