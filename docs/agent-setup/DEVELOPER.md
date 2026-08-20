# Klarbog — udvikler agent-bootstrap (ikke slutbruger)

Til AI’er der **udvikler** Klarbog i kilde-repoet.  
Slutbrugere / “tal med det installerede produkt” → se **`prompt.md`** i samme mappe.

```bash
cd /path/to/klarbog   # din clone
git checkout rust-dev
./scripts/verify.sh   # forvent VERIFY_OK
cargo build -p klarbog-mcp --release
cargo run -p klarbog-cli -- demo
# optional: ./scripts/package-dev.sh   # dist/ + sha256; then ./scripts/contract-smoke.sh
```

Env names: root [`.env.example`](../../.env.example) (Klarbog `KLARBOG_*` only).  
Skills under `docs/skills/` (incl. `moms-suggest.md`). ADR under `docs/adr/`.  
Swarm: `swarm/ROADMAP.md`. DEV only. Penge = i64. Journal = to-fase confirm.

Klarbog er en Rust-port af [Rentemester](https://github.com/mikkelkrogsholm/rentemester)
(Mikkel Krogsholm m.fl., MIT) — se root `README.md` / `NOTICE`.

