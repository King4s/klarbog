---
title: "kurt-write-policy-2026-08-19"
tags: ["kurt","policy","klarbog"]
project: "Klarbog"
description: ""
updated: "2026-08-19"
---

# Kurt write policy (2026-08-19)

Owner: no restrictions on WHAT may be written to Kurt; WHERE remains enforced.

- Removed write-time `source_path_in_scope` / `KURT_WRITE_PATH_OUT_OF_PROJECT_SCOPE`.
- Kept: under `/opt/pellucid-software`, `.md`, nested `<area>/.../*.md`, safe area slug.
- `project` is metadata + API-key allowlist only.
- Query/view may still use project source scope.
- Redeployed `kurt-rust:prod` on Thor; health 200.
