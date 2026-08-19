//! Default compile-time plugin registry (Meta + CRM + Invoice + Documents + RulesDk).

use klarbog_plugin::{Registry, META};
use klarbog_plugin_crm::CrmPlugin;
use klarbog_plugin_documents::DocumentsPlugin;
use klarbog_plugin_invoice::InvoicePlugin;
use klarbog_plugin_rules_dk::RULES_DK;
use std::sync::OnceLock;

static CRM: OnceLock<CrmPlugin> = OnceLock::new();
static INVOICE: OnceLock<InvoicePlugin> = OnceLock::new();
static DOCUMENTS: OnceLock<DocumentsPlugin> = OnceLock::new();

fn crm() -> &'static CrmPlugin {
    CRM.get_or_init(CrmPlugin::default)
}

fn invoice() -> &'static InvoicePlugin {
    INVOICE.get_or_init(InvoicePlugin::default)
}

fn documents() -> &'static DocumentsPlugin {
    DOCUMENTS.get_or_init(DocumentsPlugin::default)
}

pub fn default_registry() -> Registry {
    let crm = crm();
    let invoice = invoice();
    let docs = documents();
    Registry::new(vec![&META, crm, invoice, docs, &RULES_DK], vec![&RULES_DK])
        .expect("default plugin registry")
}

#[cfg(test)]
mod tests {
    use super::*;
    use klarbog_plugin::Capability;

    #[test]
    fn default_has_meta_crm_invoice_documents_rules_dk() {
        let reg = default_registry();
        let ids: Vec<_> = reg.list().map(|p| p.id()).collect();
        assert!(ids.contains(&"meta"));
        assert!(ids.contains(&"crm"));
        assert!(ids.contains(&"invoice"));
        assert!(ids.contains(&"documents"));
        assert!(ids.contains(&"rules-dk"));
        assert_eq!(reg.list_by_capability(Capability::RulesValidate).count(), 1);
        let crm_plugin = reg.list().find(|p| p.id() == "crm").unwrap();
        assert!(!crm_plugin.has_journal_write());
        let invoice_plugin = reg.list().find(|p| p.id() == "invoice").unwrap();
        assert!(!invoice_plugin.has_journal_write());
        let docs_plugin = reg.list().find(|p| p.id() == "documents").unwrap();
        assert!(!docs_plugin.has_journal_write());
    }
}
