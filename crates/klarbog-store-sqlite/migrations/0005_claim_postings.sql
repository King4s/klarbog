-- Append-only claim → journal posting links
-- (DK-INVOICE-*-BOOKKEEPING-001). Natural claim identity (not integer claim PK)
-- mirrors invoices.json; journal_entry_id is TEXT (Klarbog journal digests/ids).
-- UNIQUE on claim identity + journal_entry_id enforces one post per claim.

CREATE TABLE IF NOT EXISTS invoice_reminder_postings (
    id INTEGER PRIMARY KEY,
    invoice_id TEXT NOT NULL,
    reminder_date TEXT NOT NULL,
    journal_entry_id TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (invoice_id, reminder_date),
    FOREIGN KEY (invoice_id, reminder_date)
        REFERENCES invoice_reminders (invoice_id, reminder_date)
);

CREATE INDEX IF NOT EXISTS idx_invoice_reminder_postings_invoice
ON invoice_reminder_postings (invoice_id);

CREATE TABLE IF NOT EXISTS invoice_interest_postings (
    id INTEGER PRIMARY KEY,
    invoice_id TEXT NOT NULL,
    claim_date TEXT NOT NULL,
    reference_rate_bps INTEGER NOT NULL,
    journal_entry_id TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (invoice_id, claim_date, reference_rate_bps),
    FOREIGN KEY (invoice_id, claim_date, reference_rate_bps)
        REFERENCES invoice_interest_claims (invoice_id, claim_date, reference_rate_bps)
);

CREATE INDEX IF NOT EXISTS idx_invoice_interest_postings_invoice
ON invoice_interest_postings (invoice_id);

CREATE TABLE IF NOT EXISTS invoice_compensation_postings (
    id INTEGER PRIMARY KEY,
    invoice_id TEXT NOT NULL,
    claim_date TEXT NOT NULL,
    journal_entry_id TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (invoice_id, claim_date),
    FOREIGN KEY (invoice_id)
        REFERENCES invoice_compensation_claims (invoice_id)
);

CREATE INDEX IF NOT EXISTS idx_invoice_compensation_postings_invoice
ON invoice_compensation_postings (invoice_id);

CREATE TRIGGER IF NOT EXISTS invoice_reminder_postings_no_update
BEFORE UPDATE ON invoice_reminder_postings
BEGIN
    SELECT RAISE(
        ABORT,
        'invoice reminder postings are append-only; reverse the journal entry instead'
    );
END;

CREATE TRIGGER IF NOT EXISTS invoice_reminder_postings_no_delete
BEFORE DELETE ON invoice_reminder_postings
BEGIN
    SELECT RAISE(
        ABORT,
        'invoice reminder postings are append-only; reverse the journal entry instead'
    );
END;

CREATE TRIGGER IF NOT EXISTS invoice_interest_postings_no_update
BEFORE UPDATE ON invoice_interest_postings
BEGIN
    SELECT RAISE(
        ABORT,
        'invoice interest postings are append-only; reverse the journal entry instead'
    );
END;

CREATE TRIGGER IF NOT EXISTS invoice_interest_postings_no_delete
BEFORE DELETE ON invoice_interest_postings
BEGIN
    SELECT RAISE(
        ABORT,
        'invoice interest postings are append-only; reverse the journal entry instead'
    );
END;

CREATE TRIGGER IF NOT EXISTS invoice_compensation_postings_no_update
BEFORE UPDATE ON invoice_compensation_postings
BEGIN
    SELECT RAISE(
        ABORT,
        'invoice compensation postings are append-only; reverse the journal entry instead'
    );
END;

CREATE TRIGGER IF NOT EXISTS invoice_compensation_postings_no_delete
BEFORE DELETE ON invoice_compensation_postings
BEGIN
    SELECT RAISE(
        ABORT,
        'invoice compensation postings are append-only; reverse the journal entry instead'
    );
END;

INSERT INTO schema_version (version) VALUES (5);
