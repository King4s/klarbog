# Active swarm status

- **Wave 3 (productize) landed** — slices 22–29 queued done; tip VERIFY_OK when merged
- **Wave 4** next: live-fixture hardening, webhook→apply one-shot, retention bogføring notes, MCP erase-party, soft-linegate keep-under-300
- Do not stop between slices unless `swarm/STOP` or real blocker
- Verify gate: `./scripts/verify.sh` (`VERIFY_OK`)
- Tip: no sister-product / internal host names in the public tree
- Tip: serialize env-mutating tests with tokio Mutex
