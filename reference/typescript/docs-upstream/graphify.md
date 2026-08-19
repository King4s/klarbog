# Graphify

Rentemester has a committed Graphify code graph in `graphify-out/graph.json`.

Use it before broad codebase searches:

```bash
graphify query "how does invoice issuing connect to ledger posting?"
graphify explain "postIssuedInvoiceToLedger"
graphify path "issueInvoice()" "postJournalEntry()"
```

The current graph was built with AST-only extraction:

```bash
graphify update . --no-cluster
```

Full semantic extraction, `GRAPH_REPORT.md`, and `graph.html` require an LLM
API key in the environment, for example `GEMINI_API_KEY`, `OPENAI_API_KEY`,
`ANTHROPIC_API_KEY`, `MOONSHOT_API_KEY`, or `DEEPSEEK_API_KEY`:

```bash
graphify extract .
```

Local git hooks are installed in `.git/hooks` to refresh the graph after
commits and checkouts. Those hooks are local machine state, not committed repo
files.
