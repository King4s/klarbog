# Active swarm status

- **Wave 3 landed** on `rust-dev` (`30d0021`)
- **Wave 4 (deepen) in flight** — MCP erase, stripe→apply preview, chart stub, backup/erase audit, linegate, contract smoke
- Do not stop between slices unless `swarm/STOP` or real blocker
- Verify gate: `./scripts/verify.sh` (`VERIFY_OK`)
- Tip: no sister-product / internal host names in the public tree; ignore `slice-*.md` scratch notes
- Tip: serialize env-mutating tests with tokio Mutex
