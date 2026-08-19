# Kurt write recipe (Klarbog)

Kurt `POST /api/agent/write` requires:

1. `path` under write root `/opt/pellucid-software`, ending in `.md`
2. `project` whose source scope covers that relative path

## Preferred

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

Project `Klarbog` is registered with path `/opt/pellucid-software/klarbog`.

## Also works

`project: "SeaAid.Me"` with `path: "klarbog/..."` — Klarbog is in SeaAid.Me
`KURT_PROJECT_EXTRA_PATHS` / code defaults.

## Does not work

Writing `docs/...` under SeaAid.Me (that is seaconnect-relative scope, not write-root),
or writing pellucid-software paths outside the project’s source prefixes.
