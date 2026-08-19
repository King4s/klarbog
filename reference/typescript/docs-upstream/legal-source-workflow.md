# Legal source workflow

Rentemester must be built from cited Danish sources, not vibes.

Loop:
1. Add source metadata to `sources/legal-sources.json`.
2. Run `bun run scripts/download-legal-sources.ts`.
3. Store downloaded XML/HTML/PDF under `sources/downloaded/` with SHA-256 index.
4. Encode only narrow, testable rules in `rules/dk/*.yaml`.
5. Add golden tests for each rule before relying on it.
6. Implement deterministic validators in core. Agents may suggest; validators decide.

The source corpus is intentionally local-first so builds and audits can cite exact bytes, hashes, and download dates.
