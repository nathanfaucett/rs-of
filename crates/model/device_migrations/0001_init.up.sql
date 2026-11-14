CREATE TABLE IF NOT EXISTS local_device_copy_cleanup (device_id UUID PRIMARY KEY, cleanup_id UUID NOT NULL, completed BOOLEAN NOT NULL);
CREATE TABLE IF NOT EXISTS devices (id UUID PRIMARY KEY, owner_subject TEXT NOT NULL, name TEXT, public_key TEXT, address TEXT, state INTEGER, approved_at INTEGER, revoked_at INTEGER, created_at INTEGER, updated_at INTEGER);
CREATE UNIQUE INDEX IF NOT EXISTS devices_public_key ON devices (public_key);
CREATE TABLE IF NOT EXISTS local_device_enrollments (device_id UUID PRIMARY KEY, enrollment_code_hash BLOB, enrollment_expires_at INTEGER, pairing_accepting_public_key TEXT);
CREATE TABLE IF NOT EXISTS device_selection_policies (device_id UUID PRIMARY KEY, owner_subject TEXT NOT NULL, application_id UUID, selected_kind TEXT, selected_id UUID, admin_allowed BOOLEAN NOT NULL);
CREATE TABLE IF NOT EXISTS device_resource_selections (id UUID PRIMARY KEY, device_id UUID NOT NULL, owner_subject TEXT NOT NULL, application_id UUID NOT NULL, selected_kind TEXT NOT NULL, selected_id UUID NOT NULL, selected BOOLEAN NOT NULL);
