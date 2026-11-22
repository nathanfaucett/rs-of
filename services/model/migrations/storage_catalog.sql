CREATE TABLE IF NOT EXISTS databases (
    id UUID PRIMARY KEY,
    owner_subject TEXT NOT NULL,
    application_id UUID NOT NULL,
    resource_id UUID NOT NULL,
    name TEXT,
    initializing BOOLEAN NOT NULL DEFAULT FALSE,
    deleted BOOLEAN NOT NULL DEFAULT FALSE
);
CREATE INDEX IF NOT EXISTS databases_scope ON databases (owner_subject, application_id, resource_id);
