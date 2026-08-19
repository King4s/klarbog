---
title: "status"
tags: ["klarbog","rust","dev","bootstrap"]
project: "Klarbog"
description: ""
updated: "2026-08-19"
---

# Klarbog status (2026-08-19)

Branch `rust-dev` at `/opt/pellucid-software/klarbog`. DEV-only. Pushed to origin.

HEAD: `34dd202` (tests/verifier). Prior: `8eec46e` scaffold, `785f91d` confirm/CRM.

## Passed
- cargo test --workspace green
- cargo clippy -D warnings, fmt --check
- line gate (hard 400); no f32/f64 in crates
- swarm_dispatch OK; swarm/STOP absent by default

## Crates
klarbog-types, klarbog-journal, klarbog-store-sqlite, klarbog-core,
klarbog-plugin, klarbog-plugin-crm (no journal-write), klarbog-api,
klarbog-mcp, klarbog-cli

Money is i64 minor units. Double-entry is mandatory. API 127.0.0.1:3195.
