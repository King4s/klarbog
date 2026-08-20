---
title: "status"
tags: ["klarbog","rust","dev"]
project: "Klarbog"
description: ""
updated: "2026-08-20"
---

# Klarbog status

Branch `rust-dev`. DEV-only.

Wave 32: invoice mark-paid / mark-part-paid optional ConfirmStore `preview`
(HTTP + MCP; default suggestion-only; never auto-commit).
Wave 35: offline `contract_smoke` covers mark-paid / mark-part-paid ConfirmStore
`preview:true` (default has none). MCP invoice tests soft-split
(`invoice_tests` + `invoice_preview_tests`).
Product AI prompt: `docs/agent-setup/prompt.md`.
Gate: `./scripts/verify.sh` → `VERIFY_OK`.
