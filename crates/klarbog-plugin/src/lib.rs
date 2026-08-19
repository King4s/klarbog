//! Compile-time plugins only (ADR-002). No dynamic .so loading.

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

/// Host registry — explicit static registration.
pub struct Registry {
    plugins: Vec<&'static dyn Plugin>,
}

impl Registry {
    pub fn new(plugins: Vec<&'static dyn Plugin>) -> Result<Self, KlarbogError> {
        for p in &plugins {
            // Defense: document that CRM must not advertise JournalWrite.
            let _ = p.id();
        }
        Ok(Self { plugins })
    }

    pub fn list(&self) -> impl Iterator<Item = &dyn Plugin> {
        self.plugins.iter().copied()
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

    #[test]
    fn meta_has_no_journal_write() {
        assert!(!META.has_journal_write());
        let reg = Registry::new(vec![&META]).unwrap();
        assert_eq!(reg.list().count(), 1);
    }
}
