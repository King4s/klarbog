ALTER TABLE journal_entries ADD COLUMN retain_until TEXT;

INSERT INTO schema_version (version) VALUES (2);
