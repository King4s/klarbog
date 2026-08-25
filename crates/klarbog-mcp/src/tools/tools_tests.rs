use super::*;
use klarbog_core::default_registry;

#[test]
fn tools_list_names() {
    let listed = tools_list();
    let tools = listed["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.contains(&"klarbog_health"));
    assert!(names.contains(&"klarbog_status"));
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
    assert!(names.contains(&"revolut_oauth_start"));
    assert!(names.contains(&"revolut_oauth_callback"));
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
    assert!(names.contains(&"invoice_compensation_calc"));
    assert!(names.contains(&"invoice_claim_compensation"));
    assert!(names.contains(&"invoice_post_compensation_preview"));
    assert!(names.contains(&"invoice_interest_calc"));
    assert!(names.contains(&"invoice_claim_interest"));
    assert!(names.contains(&"invoice_post_interest_preview"));
    assert!(names.contains(&"invoice_send_email"));
    assert!(names.contains(&"journal_post_preview"));
    assert!(names.contains(&"journal_post_commit"));
    assert!(names.contains(&"journal_moms_post_suggestion"));
    assert!(names.contains(&"rules_chart_list"));
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

#[test]
fn status_includes_allowlist_and_plugins() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = ConfirmStore::default();
    let registry = default_registry();
    let root = Path::new("/tmp/klarbog-status-test");
    let env = handle_tool_call("klarbog_status", &json!({}), &store, root, &registry, &rt);
    assert!(env.ok);
    let data = env.data.expect("status data");
    assert_eq!(data["mode"], "dev");
    assert_eq!(data["bind"], "stdio");
    assert_eq!(
        data["allowlist_root"].as_str().unwrap(),
        root.to_str().unwrap()
    );
    let plugins = data["plugins"].as_array().expect("plugins");
    let ids: Vec<&str> = plugins.iter().filter_map(|p| p["id"].as_str()).collect();
    assert!(ids.contains(&"meta"));
    assert!(ids.contains(&"crm"));
    assert!(ids.contains(&"rules-dk"));
}
