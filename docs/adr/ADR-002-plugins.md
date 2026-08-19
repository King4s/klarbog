# ADR-002: Plugin mechanism

## Status
Accepted (Claude CONDITIONAL GO IMPORTANT)

## Decision
Plugins are **compile-time** Rust crates implementing `klarbog_plugin::Plugin`.
No dynamic `.so` / `dlopen`. Registration is explicit in the host binary.

CRM (`plugins/crm` when added) **must not** depend on journal-write APIs.
Compile-time separation + runtime negative tests.

## Consequences
Safer for autonomous agents; capability boundaries are linker-enforced.
