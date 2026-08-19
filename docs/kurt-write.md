# Kurt write recipe (Klarbog)

Owner rule: **no project-scope limit on WHAT** you write. **WHERE** is enforced.

`POST /api/agent/write` requires:

1. `path` under write root `/opt/pellucid-software`, ending in `.md`
2. Nested layout: `<area>/.../<file>.md` (not a top-level file under the write root)
3. First segment (`area`) is a safe slug (e.g. `klarbog`, `SeaAid.Me`)
4. `project` is metadata + API-key allowlist only — it does **not** gate the path

## Example

```bash
curl -H "x-api-key: $KURT_KEY" -H "Content-Type: application/json" \
  http://127.0.0.1:3115/api/agent/write \
  -d '{
    "project": "Klarbog",
    "path": "klarbog/docs/status.md",
    "content": "# status\n",
    "tags": ["klarbog"]
  }'
```

Writing `klarbog/...` with `project: "SeaAid.Me"` (or any authorized project name) is allowed.
Query/search may still filter by project scope; write only places files under the write root.
