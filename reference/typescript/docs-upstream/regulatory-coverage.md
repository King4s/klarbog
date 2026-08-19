# Regulatory coverage

Regulatory coverage is like code coverage, but it measures how much of the
cited Danish legislation is traceably implemented in Rentemester's rules and
code — not how many lines run, but how many statutory provisions are claimed.

## Citation schema

A rule in `rules/dk/*.yaml` may carry an optional `provisions:` block (by
convention placed right after `source_id:`). Each entry cites one statutory
provision at stk-level or finer and pins the legislation text it was reviewed
against:

```yaml
    source_id: DK-BILAG-OPBEVARING-2023-1383
    provisions:
      - ref: "§ 1, stk. 1"
        text_hash: "sha256:abc123..."
      - ref: "§ 1, stk. 1, nr. 4"
        text_hash: "sha256:def456..."
```

`ref` is a human-readable provision reference; `text_hash` is copied verbatim
from the provision extractor (`src/core/legal-provisions.ts`). A rule with no
`provisions:` block is "uncited".

## Scope manifest

`sources/scope.yaml` declares which statutory provisions are *in scope* for
Rentemester. Without it the coverage denominator would count every operative
provision in every downloaded law — including hundreds (momsloven alone has
807) that Rentemester never implements — which makes the headline number
misleading.

The manifest maps each source id to either `in_scope: all` (the whole document
is relevant) or a list of §-ranges:

```yaml
version: dk-scope-v0.0.1
sources:
  DK-BILAG-OPBEVARING-2023-1383:
    in_scope: all
  DK-RENTELOVEN-2014-459:
    in_scope:
      - "§ 1-§ 9b"
  DK-MOMSLOVEN-2024-209:
    in_scope:
      - "§ 23"
      - "§ 37-§ 42"
```

A range entry is either a single paragraf (`"§ 46"`) or an inclusive range
(`"§ 37-§ 42"`). Paragraf identifiers may carry a letter suffix (`3a`, `9b`);
range membership is numeric-aware, so `3a` lies between `3` and `4`. Every
downloaded source must appear in the manifest. The file is a hand-parsed,
deterministic YAML subset — it is **reviewable**: the project owner makes the
final scope call, the seeded ranges are a starting point.

**The headline coverage metric is a self-attestation, not an objective fact.**
Its denominator is whatever `sources/scope.yaml` declares — narrowing a range
raises the percentage. The scope checks only guarantee that no *cited*
provision falls *outside* the declared scope; they cannot stop scope being
drawn too narrowly. Always read the in-scope figure together with the raw
corpus-wide figure the report prints, and review `sources/scope.yaml` and its
diffs as deliberately as the rules themselves.

## What the engine checks

`computeRegulatoryCoverage()` (`src/core/regulatory-coverage.ts`) cross-checks
every citation against the deterministic LexDania extractor:

- **Closure** — a citation must resolve to a provision in the rule's declared
  `source_id`. A `ref` that does not resolve, or resolves only in a different
  source, is a closure error.
- **Drift** — the cited `text_hash` must equal the extractor's current hash for
  that provision. A mismatch means the legislation text changed since the rule
  was reviewed.
- **Scope** — three hard-error checks against `sources/scope.yaml`:
  (a) every downloaded source must appear in the manifest;
  (b) every range endpoint must reference a paragraf that exists in that
  source; (c) every operative provision *cited* by a rule must be *in scope* —
  a citation outside declared scope is a scope-manifest gap.
- **Coverage** — the headline metric is *in-scope cited / in-scope operative*:
  per source and overall, the fraction of in-scope `operative` provisions cited
  by at least one rule. The raw corpus-wide `operativeCount`/`citedCount` are
  kept too, for transparency, and the report states the out-of-scope count.

## Running it

```bash
rentemester reg coverage              # human summary
rentemester reg coverage --format json
rentemester reg coverage --out REGULATORY_COVERAGE.md
```

The command is repo-static (no `--company`). The gate test
`tests/unit/regulatory-coverage.test.ts` hard-fails on any closure, drift or
scope error. The coverage percentage is not gated — it is a tracked metric,
not a wall.

## Reviewing citations

```bash
rentemester reg citations             # Markdown review aid to stdout
rentemester reg citations --out CITATIONS.md
```

`reg citations` emits a deterministic Markdown review aid: for every rule that
has citations (sorted by `rule_id`), it prints the rule id, its `name` and
`explanation`, and for each citation the `ref` plus the **verbatim statutory
text** of that provision. This lets the project owner verify, by eye, that
each rule is mapped to the correct paragraph. Like `reg coverage` it is
repo-static and carries no timestamps.
