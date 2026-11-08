CREATE TABLE IF NOT EXISTS sync_selections (
    id UUID PRIMARY KEY,
    device_id UUID NOT NULL,
    resource_id UUID NOT NULL,
    application_id UUID NOT NULL,
    owner_subject TEXT NOT NULL,
    selected_at INTEGER NOT NULL,
    revoked_at INTEGER
);
CREATE UNIQUE INDEX IF NOT EXISTS sync_selections_device_resource ON sync_selections (device_id, resource_id);
CREATE INDEX IF NOT EXISTS sync_selections_resource ON sync_selections (resource_id);
CREATE INDEX IF NOT EXISTS sync_selections_device ON sync_selections (device_id);
