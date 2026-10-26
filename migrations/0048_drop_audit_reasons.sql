-- Administration panel: changes no longer carry a typed reason. The audit records who acted,
-- what changed (action, target, before/after) and when; content edits and versions likewise.
ALTER TABLE admin_audit_events DROP COLUMN reason;
ALTER TABLE content_edits DROP COLUMN reason;
ALTER TABLE content_versions DROP COLUMN reason;
