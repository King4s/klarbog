-- Invoice claim registration audit (DK-INVOICE-REMINDER-FEE-001,
-- DK-INVOICE-LATE-INTEREST-REGISTER-001,
-- DK-INVOICE-LATE-COMPENSATION-REGISTER-001).
-- invoice_id is TEXT (Klarbog Rust invoice ids). Money fields are INTEGER øre.
-- Rates are INTEGER basis points (bps). Append-only; natural UNIQUE keys for
-- idempotent dual-write after JSON register.

CREATE TABLE IF NOT EXISTS invoice_reminders (
    id INTEGER PRIMARY KEY,
    invoice_id TEXT NOT NULL,
    reminder_date TEXT NOT NULL,
    fee_amount_minor INTEGER NOT NULL CHECK (fee_amount_minor > 0),
    currency TEXT NOT NULL DEFAULT 'DKK',
    note TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (invoice_id, reminder_date)
);

CREATE INDEX IF NOT EXISTS idx_invoice_reminders_invoice
ON invoice_reminders (invoice_id);

CREATE TABLE IF NOT EXISTS invoice_compensation_claims (
    id INTEGER PRIMARY KEY,
    invoice_id TEXT NOT NULL UNIQUE,
    claim_date TEXT NOT NULL,
    amount_minor INTEGER NOT NULL CHECK (amount_minor > 0),
    note TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS invoice_interest_claims (
    id INTEGER PRIMARY KEY,
    invoice_id TEXT NOT NULL,
    claim_date TEXT NOT NULL,
    reference_rate_bps INTEGER NOT NULL,
    annual_interest_rate_bps INTEGER NOT NULL,
    reference_rate_source TEXT NOT NULL DEFAULT 'manual-override'
        CHECK (reference_rate_source IN ('statutory-table', 'manual-override')),
    claimable_days INTEGER NOT NULL,
    principal_open_minor INTEGER NOT NULL,
    amount_minor INTEGER NOT NULL CHECK (amount_minor > 0),
    note TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (invoice_id, claim_date, reference_rate_bps)
);

CREATE INDEX IF NOT EXISTS idx_invoice_interest_claims_invoice
ON invoice_interest_claims (invoice_id);

CREATE TRIGGER IF NOT EXISTS invoice_reminders_no_update
BEFORE UPDATE ON invoice_reminders
BEGIN
    SELECT RAISE(
        ABORT,
        'invoice reminders are append-only; add a later reminder or corrective note instead'
    );
END;

CREATE TRIGGER IF NOT EXISTS invoice_reminders_no_delete
BEFORE DELETE ON invoice_reminders
BEGIN
    SELECT RAISE(
        ABORT,
        'invoice reminders are append-only; add a corrective note instead'
    );
END;

CREATE TRIGGER IF NOT EXISTS invoice_compensation_claims_no_update
BEFORE UPDATE ON invoice_compensation_claims
BEGIN
    SELECT RAISE(
        ABORT,
        'invoice compensation claims are append-only; add a correcting note instead'
    );
END;

CREATE TRIGGER IF NOT EXISTS invoice_compensation_claims_no_delete
BEFORE DELETE ON invoice_compensation_claims
BEGIN
    SELECT RAISE(
        ABORT,
        'invoice compensation claims are append-only; add a correcting note instead'
    );
END;

CREATE TRIGGER IF NOT EXISTS invoice_interest_claims_no_update
BEFORE UPDATE ON invoice_interest_claims
BEGIN
    SELECT RAISE(
        ABORT,
        'invoice interest claims are append-only; add a correcting note instead'
    );
END;

CREATE TRIGGER IF NOT EXISTS invoice_interest_claims_no_delete
BEFORE DELETE ON invoice_interest_claims
BEGIN
    SELECT RAISE(
        ABORT,
        'invoice interest claims are append-only; add a correcting note instead'
    );
END;

INSERT INTO schema_version (version) VALUES (4);
