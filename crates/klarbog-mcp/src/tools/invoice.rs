//! Invoice lifecycle MCP tools (read-only journal preview).

use super::auth::{authorize_company, map_core_error, parse_actor, parse_company};
use klarbog_plugin_invoice::{
    mark_paid_preview, mark_part_paid_preview, InvoiceConfig, InvoiceError, InvoiceId,
};
use klarbog_types::Envelope;
use serde_json::{json, Value};
use std::path::Path;

fn map_invoice(err: InvoiceError) -> Envelope<Value> {
    Envelope::err([err.to_string()])
}

pub async fn invoice_mark_paid_preview(args: &Value, allowlist_root: &Path) -> Envelope<Value> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let company = match parse_company(args) {
        Ok(c) => c,
        Err(e) => return Envelope::err([e]),
    };
    let invoice_id = match args.get("invoice_id").and_then(|v| v.as_str()) {
        Some(id) if !id.is_empty() => InvoiceId::new(id),
        _ => return Envelope::err(["missing invoice_id"]),
    };
    let path = match authorize_company(allowlist_root, &company, &actor).await {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    match mark_paid_preview(&path, &invoice_id, &actor, &InvoiceConfig::default()) {
        Ok((invoice, journal_entry)) => Envelope::ok(json!({
            "invoice": invoice,
            "journal_entry": journal_entry,
        })),
        Err(e) => map_invoice(e),
    }
}

pub async fn invoice_mark_part_paid_preview(
    args: &Value,
    allowlist_root: &Path,
) -> Envelope<Value> {
    let actor = match parse_actor(args) {
        Ok(a) => a,
        Err(e) => return Envelope::err([e]),
    };
    let company = match parse_company(args) {
        Ok(c) => c,
        Err(e) => return Envelope::err([e]),
    };
    let invoice_id = match args.get("invoice_id").and_then(|v| v.as_str()) {
        Some(id) if !id.is_empty() => InvoiceId::new(id),
        _ => return Envelope::err(["missing invoice_id"]),
    };
    let amount_minor = match args.get("amount_minor").and_then(|v| v.as_i64()) {
        Some(a) => a,
        None => return Envelope::err(["missing amount_minor"]),
    };
    let path = match authorize_company(allowlist_root, &company, &actor).await {
        Ok(p) => p,
        Err(e) => return map_core_error(e),
    };
    match mark_part_paid_preview(
        &path,
        &invoice_id,
        amount_minor,
        &actor,
        &InvoiceConfig::default(),
    ) {
        Ok((invoice, journal_entry)) => Envelope::ok(json!({
            "invoice": invoice,
            "journal_entry": journal_entry,
        })),
        Err(e) => map_invoice(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use klarbog_core::init_company;
    use klarbog_plugin_crm::upsert_party;
    use klarbog_plugin_invoice::{
        create_draft_from_new, patch_status, InvoiceKind, InvoiceStatus, NewLine,
    };
    use klarbog_types::Actor;
    use tempfile::tempdir;

    #[tokio::test]
    async fn mcp_mark_paid_preview() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        std::fs::create_dir_all(&co).unwrap();
        let owner = Actor::user("owner");
        init_company(&co, "Demo", &owner).await.unwrap();
        let party = upsert_party(&co, None, "Buyer".into()).unwrap();
        let invoice = create_draft_from_new(
            &co,
            party.id,
            InvoiceKind::Sale,
            vec![NewLine {
                description: "Item".into(),
                amount_minor: 1000,
                currency: "DKK".into(),
            }],
        )
        .unwrap();
        patch_status(&co, &invoice.id, InvoiceStatus::Sent).unwrap();
        let args = json!({
            "company": co.to_string_lossy(),
            "invoice_id": invoice.id.to_string(),
            "actor_kind": "user",
            "actor_id": "owner",
        });
        let env = invoice_mark_paid_preview(&args, dir.path()).await;
        assert!(env.ok);
        let data = env.data.unwrap();
        assert_eq!(data["invoice"]["status"], "paid");
        assert!(data["journal_entry"]["legs"].as_array().unwrap()[0]["party_id"].is_string());
    }

    #[tokio::test]
    async fn mcp_mark_part_paid_preview() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        std::fs::create_dir_all(&co).unwrap();
        let owner = Actor::user("owner");
        init_company(&co, "Demo", &owner).await.unwrap();
        let party = upsert_party(&co, None, "Buyer".into()).unwrap();
        let invoice = create_draft_from_new(
            &co,
            party.id,
            InvoiceKind::Sale,
            vec![NewLine {
                description: "Item".into(),
                amount_minor: 10_000,
                currency: "DKK".into(),
            }],
        )
        .unwrap();
        patch_status(&co, &invoice.id, InvoiceStatus::Sent).unwrap();
        let args = json!({
            "company": co.to_string_lossy(),
            "invoice_id": invoice.id.to_string(),
            "amount_minor": 3_000,
            "actor_kind": "user",
            "actor_id": "owner",
        });
        let env = invoice_mark_part_paid_preview(&args, dir.path()).await;
        assert!(env.ok);
        let data = env.data.unwrap();
        assert_eq!(data["invoice"]["status"], "part_paid");
        assert!(data["journal_entry"]["legs"].as_array().unwrap()[0]["party_id"].is_string());
        let amount = &data["journal_entry"]["legs"][0]["amount"];
        let units = amount
            .as_i64()
            .or_else(|| amount.get("units").and_then(|u| u.as_i64()));
        assert_eq!(units, Some(3_000));
    }
}
