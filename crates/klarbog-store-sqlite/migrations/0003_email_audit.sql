-- Email delivery audit (DK-EMAIL-DELIVERY-001). Append-only; message_id is
-- the idempotency key. SMTP credentials are never stored — only smtp_host.
-- invoice_id is TEXT (Klarbog Rust invoice ids), not TS INTEGER document id.

CREATE TABLE IF NOT EXISTS audit_log (
    id INTEGER PRIMARY KEY,
    event_type TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT,
    message TEXT NOT NULL,
    actor TEXT NOT NULL DEFAULT 'system',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS email_send_log (
    id INTEGER PRIMARY KEY,
    invoice_id TEXT NOT NULL,
    invoice_no TEXT,
    kind TEXT NOT NULL CHECK (kind IN ('invoice', 'reminder')),
    recipient TEXT NOT NULL,
    sender TEXT NOT NULL,
    subject TEXT NOT NULL,
    message_id TEXT NOT NULL UNIQUE,
    body_sha256 TEXT NOT NULL,
    smtp_host TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_email_send_log_invoice
ON email_send_log (invoice_id);

CREATE TRIGGER IF NOT EXISTS email_send_log_no_update
BEFORE UPDATE ON email_send_log
BEGIN
    SELECT RAISE(
        ABORT,
        'email send log is append-only audit data; record a new send instead'
    );
END;

CREATE TRIGGER IF NOT EXISTS email_send_log_no_delete
BEFORE DELETE ON email_send_log
BEGIN
    SELECT RAISE(
        ABORT,
        'email send log is append-only audit data; record a new send instead'
    );
END;

CREATE TRIGGER IF NOT EXISTS audit_log_no_update
BEFORE UPDATE ON audit_log
BEGIN
    SELECT RAISE(ABORT, 'audit_log is append-only');
END;

CREATE TRIGGER IF NOT EXISTS audit_log_no_delete
BEFORE DELETE ON audit_log
BEGIN
    SELECT RAISE(ABORT, 'audit_log is append-only');
END;

INSERT INTO schema_version (version) VALUES (3);
