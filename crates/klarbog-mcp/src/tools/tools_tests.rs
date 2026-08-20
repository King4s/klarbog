use super::*;
use klarbog_core::default_registry;

#[test]
fn tools_list_names() {
    let listed = tools_list();
    let tools = listed["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.contains(&"klarbog_health"));
    assert!(names.contains(&"crm_upsert_party"));
    assert!(names.contains(&"crm_list_parties"));
    assert!(names.contains(&"documents_attach"));
    assert!(names.contains(&"documents_list"));
    assert!(names.contains(&"documents_delete"));
    assert!(names.contains(&"exceptions_raise"));
    assert!(names.contains(&"exceptions_list"));
    assert!(names.contains(&"exceptions_set_open"));
    assert!(names.contains(&"bank_import_preview"));
    assert!(names.contains(&"bank_stripe_consume"));
    assert!(names.contains(&"bank_reconcile_suggest"));
    assert!(names.contains(&"bank_reconcile_apply"));
    assert!(names.contains(&"revolut_oauth_refresh"));
    assert!(names.contains(&"bank_stripe_reconcile_suggest"));
    assert!(names.contains(&"bank_stripe_reconcile_apply_preview"));
    assert!(names.contains(&"retention_get"));
    assert!(names.contains(&"retention_purge"));
    assert!(names.contains(&"backup_manifest"));
    assert!(names.contains(&"gdpr_export"));
    assert!(names.contains(&"gdpr_erase_party"));
    assert!(names.contains(&"invoice_create_draft"));
    assert!(names.contains(&"invoice_list"));
    assert!(names.contains(&"invoice_patch_status"));
    assert!(names.contains(&"invoice_mark_paid_preview"));
    assert!(names.contains(&"invoice_mark_part_paid_preview"));
    assert!(names.contains(&"journal_post_preview"));
    assert!(names.contains(&"journal_post_commit"));
    assert!(names.contains(&"journal_moms_post_suggestion"));
}

#[test]
fn health_via_handler() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = ConfirmStore::default();
    let registry = default_registry();
    let env = handle_tool_call(
        "klarbog_health",
        &json!({}),
        &store,
        Path::new("/tmp"),
        &registry,
        &rt,
    );
    assert!(env.ok);
}
