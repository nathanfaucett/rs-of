CREATE TABLE IF NOT EXISTS roles (
    id UUID PRIMARY KEY,
    application_id UUID,
    name TEXT,
    description TEXT,
    revoked_at INTEGER,
    created_at INTEGER,
    updated_at INTEGER
);
CREATE UNIQUE INDEX IF NOT EXISTS roles_application_name ON roles (application_id, name);

CREATE TABLE IF NOT EXISTS permissions (
    id UUID PRIMARY KEY,
    application_id UUID,
    name TEXT,
    description TEXT,
    revoked_at INTEGER,
    created_at INTEGER,
    updated_at INTEGER
);
CREATE UNIQUE INDEX IF NOT EXISTS permissions_application_name ON permissions (application_id, name);

CREATE TABLE IF NOT EXISTS role_permissions (
    id UUID PRIMARY KEY,
    role_id UUID,
    permission_id UUID,
    revoked_at INTEGER,
    created_at INTEGER,
    updated_at INTEGER
);
CREATE UNIQUE INDEX IF NOT EXISTS role_permissions_role_permission ON role_permissions (role_id, permission_id);

CREATE TABLE IF NOT EXISTS application_user_roles (
    id UUID PRIMARY KEY,
    user_id UUID,
    application_id UUID,
    role_id UUID,
    revoked_at INTEGER,
    created_at INTEGER,
    updated_at INTEGER
);
CREATE UNIQUE INDEX IF NOT EXISTS application_user_roles_user_application_role ON application_user_roles (user_id, application_id, role_id);

