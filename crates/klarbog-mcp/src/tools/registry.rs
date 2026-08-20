//! MCP tools/list schema registry.

use serde_json::{json, Value};

pub fn tools_list() -> Value {
    json!({
        "jsonrpc": "2.0",
        "result": {
            "tools": [
                {
                    "name": "klarbog_health",
                    "description": "Health check (read-only)",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "crm_upsert_party",
                    "description": "Upsert CRM party (display_name; optional party_id). No journal write (ADR-004).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "company": {"type": "string"},
                            "display_name": {"type": "string"},
                            "party_id": {"type": "string"},
                            "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                            "actor_id": {"type": "string"}
                        },
                        "required": ["company", "display_name", "actor_kind", "actor_id"]
                    }
                },
                {
                    "name": "crm_list_parties",
                    "description": "List CRM parties, or get one when party_id is set (read-only)",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "company": {"type": "string"},
                            "party_id": {"type": "string"},
                            "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                            "actor_id": {"type": "string"}
                        },
                        "required": ["company", "actor_kind", "actor_id"]
                    }
                },
                {
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
                },
                {
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
                },
                {
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
                },
                {
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
                },
                {
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
                },
                {
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
                },
                {
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
                },
                {
                    "name": "retention_get",
                    "description": "Load company retention policy (read-only)",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "company": {"type": "string"},
                            "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                            "actor_id": {"type": "string"}
                        },
                        "required": ["company", "actor_kind", "actor_id"]
                    }
                },
                {
                    "name": "retention_purge",
                    "description": "Purge closed exceptions (and optional orphan document metadata); dry-run unless confirm:true; journal untouched",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "company": {"type": "string"},
                            "confirm": {"type": "boolean"},
                            "gc_orphan_documents": {"type": "boolean"},
                            "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                            "actor_id": {"type": "string"}
                        },
                        "required": ["company", "actor_kind", "actor_id"]
                    }
                },
                {
                    "name": "backup_manifest",
                    "description": "Write backup manifest + sha256 sidecar; returns paths and digests",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "company": {"type": "string"},
                            "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                            "actor_id": {"type": "string"}
                        },
                        "required": ["company", "actor_kind", "actor_id"]
                    }
                },
                {
                    "name": "gdpr_export",
                    "description": "Write company-scoped GDPR export v1 metadata (gdpr_export.json; no binary blobs; i64 totals; journal note)",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "company": {"type": "string"},
                            "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                            "actor_id": {"type": "string"}
                        },
                        "required": ["company", "actor_kind", "actor_id"]
                    }
                },
                {
                    "name": "gdpr_erase_party",
                    "description": "GDPR party erase preview/confirm: anonymize display_name; strip or delete docs; journal immutable (journal_refs_retained)",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "company": {"type": "string"},
                            "party_id": {"type": "string"},
                            "confirm": {"type": "boolean"},
                            "delete_documents": {"type": "boolean"},
                            "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                            "actor_id": {"type": "string"}
                        },
                        "required": ["company", "party_id", "actor_kind", "actor_id"]
                    }
                },
                {
                    "name": "invoice_mark_paid_preview",
                    "description": "Mark invoice paid for remaining balance (payment ledger) and return payment journal suggestion with party_id (no post). Rejects when remaining is 0.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "company": {"type": "string"},
                            "invoice_id": {"type": "string"},
                            "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                            "actor_id": {"type": "string"}
                        },
                        "required": ["company", "invoice_id", "actor_kind", "actor_id"]
                    }
                },
                {
                    "name": "invoice_mark_part_paid_preview",
                    "description": "Record partial payment on invoice ledger (amount_minor >0 and < remaining), set part_paid, return journal suggestion with party_id (no post). Overpay rejected.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "company": {"type": "string"},
                            "invoice_id": {"type": "string"},
                            "amount_minor": {"type": "integer"},
                            "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                            "actor_id": {"type": "string"}
                        },
                        "required": ["company", "invoice_id", "amount_minor", "actor_kind", "actor_id"]
                    }
                },
                {
                    "name": "journal_post_preview",
                    "description": "Phase-1: validate entry and return confirmation token (no write)",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "company": {"type": "string"},
                            "entry": {"type": "object"},
                            "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                            "actor_id": {"type": "string"}
                        },
                        "required": ["company", "entry", "actor_kind", "actor_id"]
                    }
                },
                {
                    "name": "journal_post_commit",
                    "description": "Phase-2: commit with confirmation token (ADR two-phase confirm)",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "company": {"type": "string"},
                            "entry": {"type": "object"},
                            "confirm_token": {"type": "string"},
                            "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                            "actor_id": {"type": "string"}
                        },
                        "required": ["company", "entry", "confirm_token", "actor_kind", "actor_id"]
                    }
                },
                {
                    "name": "journal_moms_post_suggestion",
                    "description": "Optional moms post suggestion from gross_minor when memo has #vat25 (net+vat i64 legs). Preview only — never posts.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "company": {"type": "string"},
                            "gross_minor": {"type": "integer"},
                            "memo": {"type": "string"},
                            "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                            "actor_id": {"type": "string"}
                        },
                        "required": ["company", "gross_minor", "memo", "actor_kind", "actor_id"]
                    }
                }
            ]
        }
    })
}
