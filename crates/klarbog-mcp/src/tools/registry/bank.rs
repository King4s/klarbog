//! Bank / OAuth / Stripe MCP tool schemas.

use serde_json::{json, Value};

pub fn tools() -> Vec<Value> {
    vec![
        json!({
            "name": "bank_import_preview",
            "description": "Import bank transactions (API or CSV fallback) and return draft journal entries (read-only, no post)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "provider": {"type": "string", "enum": ["generic_dk", "revolut", "stripe"]},
                    "source": {"type": "string", "enum": ["api", "csv"]},
                    "csv": {"type": "string"},
                    "currency": {"type": "string"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "provider", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "bank_stripe_consume",
            "description": "Consume Stripe webhook queue into bank draft rows (dry-run unless confirm:true; no journal post)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "confirm": {"type": "boolean"},
                    "limit": {"type": "integer"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "bank_reconcile_suggest",
            "description": "Suggest bank row ↔ open invoice matches (rows or CSV/provider). Optional raise_exceptions; never posts journal.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "rows": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "date": {"type": "string"},
                                "text": {"type": "string"},
                                "amount_minor": {"type": "integer"}
                            },
                            "required": ["date", "text", "amount_minor"]
                        }
                    },
                    "provider": {"type": "string", "enum": ["generic_dk", "revolut", "stripe"]},
                    "source": {"type": "string", "enum": ["api", "csv"]},
                    "csv": {"type": "string"},
                    "currency": {"type": "string"},
                    "raise_exceptions": {"type": "boolean"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "bank_reconcile_apply",
            "description": "Apply bank row ↔ invoice match as journal preview suggestion (no post). force requires user actor below safe threshold.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "invoice_id": {"type": "string"},
                    "row": {
                        "type": "object",
                        "properties": {
                            "date": {"type": "string"},
                            "text": {"type": "string"},
                            "amount_minor": {"type": "integer"}
                        },
                        "required": ["date", "text", "amount_minor"]
                    },
                    "row_index": {"type": "integer"},
                    "force": {"type": "boolean"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "invoice_id", "row", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "revolut_oauth_refresh",
            "description": "Refresh Revolut OAuth access token for company secrets (never returns tokens)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "bank_stripe_reconcile_suggest",
            "description": "Stripe consume → reconcile suggest (dry-run consume unless confirm_consume; no journal post)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "confirm_consume": {"type": "boolean"},
                    "limit": {"type": "integer"},
                    "raise_exceptions": {"type": "boolean"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "bank_stripe_reconcile_apply_preview",
            "description": "Stripe consume → unique safe apply + ConfirmStore preview (no journal commit)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "confirm_consume": {"type": "boolean"},
                    "force": {"type": "boolean"},
                    "limit": {"type": "integer"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "actor_kind", "actor_id"]
            }
        }),
    ]
}
