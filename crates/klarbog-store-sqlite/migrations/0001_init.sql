-- Klarbog ledger schema v1. Double-entry legs are first-class rows (ADR-004).

CREATE TABLE schema_version (
    version INTEGER NOT NULL PRIMARY KEY
);

CREATE TABLE journal_entries (
    id TEXT PRIMARY KEY NOT NULL,
    as_of TEXT NOT NULL,
    memo TEXT NOT NULL,
    actor TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    digest TEXT NOT NULL,
    prev_digest TEXT
);

CREATE TABLE journal_legs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entry_id TEXT NOT NULL REFERENCES journal_entries (id),
    account TEXT NOT NULL,
    direction TEXT NOT NULL CHECK (direction IN ('debit', 'credit')),
    amount_minor INTEGER NOT NULL,
    currency TEXT NOT NULL,
    party_id TEXT
);

INSERT INTO schema_version (version) VALUES (1);
