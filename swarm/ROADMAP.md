# Klarbog Swarm — KANONISK slice-graf (eneste nummerering)

0. Fork + Rust scaffold + swarm/ + ADR-001..005 (CRITICAL landings)
1. Money (i64) + journal (mandatory double-entry) + actor + sqlite WAL/FK/migrations
2. API + MCP + two-phase confirm + Actor/company AuthZ
3. Plugin-host (compile-time traits) + rules-dk
4. CRM plugin (no journal-write dependency)
5. Bank CSV + expense book (**+ Revolut profile**, ADR-008)
6. Invoice + party_id
7. Documents + exceptions (**+ Cloudflare R2 EU ObjectStore**, ADR-007)
8. Skills + agent-demo
9. Backup / GDPR / retention / templates (reuse ObjectStore; EU residency)

Owner add-ons (2026-08-19): Cloudflare EU (R2 first); Revolut bank import.

Advance only after verifier green. Presence of `swarm/STOP` halts dispatch.
