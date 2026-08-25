//! Retention, GDPR, invoice, and journal MCP tool schemas.

use serde_json::{json, Value};

pub fn tools() -> Vec<Value> {
    vec![
        json!({
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
        }),
        json!({
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
        }),
        json!({
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
        }),
        json!({
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
        }),
        json!({
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
        }),
        json!({
            "name": "invoice_create_draft",
            "description": "Create invoice draft (sale|purchase lines with i64 amount_minor) and return journal suggestion with party_id (no post)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "party_id": {"type": "string"},
                    "kind": {"type": "string", "enum": ["sale", "purchase"]},
                    "lines": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "description": {"type": "string"},
                                "amount_minor": {"type": "integer"},
                                "currency": {"type": "string", "description": "ISO 4217 (default DKK); non-DKK requires fx_rate_to_dkk or fx_rate_to_dkk_micro"}
                            },
                            "required": ["description", "amount_minor", "currency"]
                        }
                    },
                    "due_date": {"type": "string", "description": "Optional YYYY-MM-DD"},
                    "fx_rate_to_dkk": {"type": "string", "description": "For non-DKK: decimal rate to DKK (e.g. 7.46)"},
                    "fx_rate_to_dkk_micro": {"type": "integer", "description": "For non-DKK: rate × 1_000_000 (alternative to fx_rate_to_dkk)"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "party_id", "kind", "lines", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "invoice_list",
            "description": "List invoices, or get one when invoice_id is set (read-only)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "invoice_id": {"type": "string"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "invoice_patch_status",
            "description": "Patch invoice lifecycle status (draft|sent|part_paid|paid|void); no journal write",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "invoice_id": {"type": "string"},
                    "status": {
                        "type": "string",
                        "enum": ["draft", "sent", "part_paid", "paid", "void"]
                    },
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "invoice_id", "status", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "invoice_mark_paid_preview",
            "description": "Mark invoice paid for remaining balance (payment ledger) and return payment journal suggestion with party_id (no post). Optional preview:true issues ConfirmStore token. Rejects when remaining is 0.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "invoice_id": {"type": "string"},
                    "preview": {"type": "boolean"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "invoice_id", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "invoice_mark_part_paid_preview",
            "description": "Record partial payment on invoice ledger (amount_minor >0 and < remaining), set part_paid, return journal suggestion with party_id (no post). Optional preview:true issues ConfirmStore token. Overpay rejected.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "invoice_id": {"type": "string"},
                    "amount_minor": {"type": "integer"},
                    "preview": {"type": "boolean"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "invoice_id", "amount_minor", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "invoice_compensation_calc",
            "description": "Calculate late-compensation claim without registering (read-only). Commercial buyer + overdue required.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "invoice_id": {"type": "string"},
                    "invoice_number": {"type": "string"},
                    "as_of": {"type": "string", "description": "YYYY-MM-DD"},
                    "amount_minor": {"type": "integer", "description": "Optional fixed compensation in øre"},
                    "amount_dkk": {"type": "number", "description": "Optional fixed compensation in DKK (decimal)"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "as_of", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "invoice_claim_compensation",
            "description": "Register late-compensation claim (no journal post). Requires confirm:true. Call invoice_post_compensation_preview then journal_post_commit.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "invoice_id": {"type": "string"},
                    "invoice_number": {"type": "string"},
                    "as_of": {"type": "string", "description": "YYYY-MM-DD"},
                    "amount_minor": {"type": "integer"},
                    "amount_dkk": {"type": "number"},
                    "note": {"type": "string"},
                    "confirm": {"type": "boolean"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "as_of", "confirm", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "invoice_post_compensation_preview",
            "description": "Return journal suggestion for oldest unposted compensation claim (no post). Optional preview:true issues ConfirmStore token for journal_post_commit.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "invoice_id": {"type": "string"},
                    "invoice_number": {"type": "string"},
                    "preview": {"type": "boolean"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "invoice_interest_calc",
            "description": "Calculate late interest without registering (read-only). accrued_interest_minor is incremental since last claim.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "invoice_id": {"type": "string"},
                    "invoice_number": {"type": "string"},
                    "as_of": {"type": "string", "description": "YYYY-MM-DD"},
                    "reference_rate": {"type": "number", "description": "Nationalbank reference rate as percent (e.g. 2.65)"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "as_of", "reference_rate", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "invoice_claim_interest",
            "description": "Register late-interest claim (no journal post). Requires confirm:true. Incremental since last claim.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "invoice_id": {"type": "string"},
                    "invoice_number": {"type": "string"},
                    "as_of": {"type": "string", "description": "YYYY-MM-DD"},
                    "reference_rate": {"type": "number"},
                    "note": {"type": "string"},
                    "confirm": {"type": "boolean"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "as_of", "reference_rate", "confirm", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "invoice_post_interest_preview",
            "description": "Return journal suggestion for an unposted interest claim (no post). Omit claim_date → oldest unposted. Optional preview:true issues ConfirmStore token for journal_post_commit.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "invoice_id": {"type": "string"},
                    "invoice_number": {"type": "string"},
                    "claim_date": {"type": "string", "description": "Optional YYYY-MM-DD of specific unposted claim"},
                    "reference_rate_bps": {"type": "integer", "description": "Optional; disambiguates claims sharing claim_date"},
                    "preview": {"type": "boolean"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "actor_kind", "actor_id"]
            }
        }),
        json!({
            "name": "invoice_send_email",
            "description": "Send issued invoice or payment reminder by email (SMTP via SMTP_SYSTEM_* / KLARBOG_SMTP_FROM; dry-run when unconfigured or KLARBOG_EMAIL_DRY_RUN=1). Append-only email_send_log.jsonl; idempotent. Requires confirm:true.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "company": {"type": "string"},
                    "invoice_id": {"type": "string"},
                    "invoice_number": {"type": "string"},
                    "kind": {"type": "string", "enum": ["invoice", "reminder"]},
                    "to": {"type": "string"},
                    "confirm": {"type": "boolean"},
                    "actor_kind": {"type": "string", "enum": ["user", "agent", "system"]},
                    "actor_id": {"type": "string"}
                },
                "required": ["company", "confirm", "actor_kind", "actor_id"]
            }
        }),
        json!({
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
        }),
        json!({
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
        }),
        json!({
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
        }),
        json!({
            "name": "rules_chart_list",
            "description": "DEV rules-dk chart stub (codes + labels). Mirrors GET /api/v1/rules/chart; read-only — no journal write.",
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
    ]
}
