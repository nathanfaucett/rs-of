CREATE TABLE IF NOT EXISTS users (
    id UUID NOT NULL,
    handle TEXT,
    name TEXT,
    given_name TEXT,
    family_name TEXT,
    middle_name TEXT,
    nickname TEXT,
    profile TEXT,
    picture TEXT,
    website TEXT,
    sex INTEGER,
    birthdate TEXT,
    zoneinfo TEXT,
    locale TEXT,
    created_at INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER,
    mfa_totp_secret TEXT,
    mfa_webauthn_credentials TEXT,
    blocked INTEGER NOT NULL DEFAULT 0,
    blocked_at INTEGER,
    blocked_by UUID,
    PRIMARY KEY (id, created_at)
);
CREATE UNIQUE INDEX IF NOT EXISTS users_id ON users (id);
CREATE UNIQUE INDEX IF NOT EXISTS users_handle ON users (handle);

CREATE TABLE IF NOT EXISTS user_emails (
    id UUID PRIMARY KEY,
    user_id UUID,
    email TEXT,
    verified INTEGER,
    primary_email INTEGER,
    created_at INTEGER,
    updated_at INTEGER
);
CREATE UNIQUE INDEX IF NOT EXISTS user_emails_user_email ON user_emails (user_id, email);

CREATE TABLE IF NOT EXISTS user_phone_numbers (
    id UUID PRIMARY KEY,
    user_id UUID,
    phone_number TEXT,
    verified INTEGER,
    primary_phone INTEGER,
    created_at INTEGER,
    updated_at INTEGER
);
CREATE UNIQUE INDEX IF NOT EXISTS user_phone_numbers_user_phone ON user_phone_numbers (user_id, phone_number);

CREATE TABLE IF NOT EXISTS credentials (
    id UUID NOT NULL,
    user_id UUID NOT NULL,
    kind TEXT,
    active INTEGER,
    revoked_at INTEGER,
    created_at INTEGER,
    updated_at INTEGER,
    PRIMARY KEY (id, user_id)
);
CREATE UNIQUE INDEX IF NOT EXISTS credentials_id ON credentials (id);

CREATE TABLE IF NOT EXISTS local_credential_secrets (
    credential_id TEXT PRIMARY KEY,
    secret_hash TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS keys (
    id UUID PRIMARY KEY,
    parent_id UUID,
    entity_type INTEGER,
    entity_id UUID,
    derivation_path TEXT,
    derivation_index INTEGER NOT NULL,
    hardened INTEGER,
    name TEXT,
    revoked_at INTEGER,
    expires_at INTEGER,
    created_at INTEGER,
    updated_at INTEGER,
    public_jwk TEXT
);
CREATE UNIQUE INDEX IF NOT EXISTS keys_derivation_path ON keys (derivation_path);
CREATE UNIQUE INDEX IF NOT EXISTS keys_parent_derivation_index ON keys (parent_id, derivation_index);

CREATE TABLE IF NOT EXISTS applications (
    id UUID PRIMARY KEY,
    name TEXT,
    uri TEXT,
    description TEXT,
    created_at INTEGER,
    updated_at INTEGER
);
CREATE UNIQUE INDEX IF NOT EXISTS applications_uri ON applications (uri);

CREATE TABLE IF NOT EXISTS clients (
    id UUID PRIMARY KEY,
    application_id UUID,
    client_id TEXT,
    credential_id UUID,
    client_id_issued_at INTEGER,
    client_secret_expires_at INTEGER,
    client_name TEXT,
    client_uri TEXT,
    redirect_uris TEXT,
    client_type INTEGER,
    profile INTEGER,
    token_endpoint_auth_method INTEGER,
    allowed_grant_types TEXT,
    response_types TEXT,
    allowed_scopes TEXT,
    allowed_audiences TEXT,
    logo_uri TEXT,
    contacts TEXT,
    terms_of_service_uri TEXT,
    policy_uri TEXT,
    software_statement TEXT,
    software_id TEXT,
    software_version TEXT,
    revoked_at INTEGER,
    created_at INTEGER,
    updated_at INTEGER
);
CREATE UNIQUE INDEX IF NOT EXISTS clients_client_id ON clients (client_id);

CREATE TABLE IF NOT EXISTS local_client_credentials (
    id UUID PRIMARY KEY,
    client_id UUID NOT NULL,
    credential_id UUID NOT NULL,
    secret_hash TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS local_client_credentials_client_id ON local_client_credentials (client_id);

CREATE TABLE IF NOT EXISTS oauth2_authorization_codes (
    id UUID PRIMARY KEY,
    code_hash TEXT,
    client_id TEXT,
    user_id UUID,
    key_id UUID,
    redirect_uri TEXT,
    scopes TEXT,
    resource TEXT,
    authorization_details TEXT,
    code_challenge TEXT,
    code_challenge_method INTEGER,
    nonce TEXT,
    expires_at INTEGER,
    consumed_at INTEGER,
    created_at INTEGER,
    updated_at INTEGER
);
CREATE UNIQUE INDEX IF NOT EXISTS oauth2_authorization_codes_code_hash ON oauth2_authorization_codes (code_hash);

CREATE TABLE IF NOT EXISTS oauth2_user_consents (
    id UUID PRIMARY KEY,
    user_id UUID,
    client_id TEXT,
    redirect_uri TEXT,
    scope TEXT,
    authorization_details TEXT,
    revoked_at INTEGER,
    created_at INTEGER,
    updated_at INTEGER
);
CREATE UNIQUE INDEX IF NOT EXISTS oauth2_user_consents_user_client_redirect_scope ON oauth2_user_consents (user_id, client_id, redirect_uri, scope);

CREATE TABLE IF NOT EXISTS oauth2_refresh_tokens (
    id UUID PRIMARY KEY,
    token_hash TEXT,
    client_id UUID,
    user_id UUID,
    scopes TEXT,
    resource TEXT,
    authorization_details TEXT,
    expires_at INTEGER,
    consumed_at INTEGER,
    revoked_at INTEGER,
    created_at INTEGER,
    updated_at INTEGER
);
CREATE UNIQUE INDEX IF NOT EXISTS oauth2_refresh_tokens_token_hash ON oauth2_refresh_tokens (token_hash);

CREATE TABLE IF NOT EXISTS idp_conflict_resolutions (
    id UUID PRIMARY KEY,
    administrator_id UUID,
    table_name TEXT,
    row_id UUID,
    columns TEXT,
    created_at INTEGER
);

CREATE TABLE IF NOT EXISTS idp_replica_members (
    id UUID PRIMARY KEY,
    installation_id TEXT NOT NULL,
    member_id UUID NOT NULL,
    endpoint_id TEXT NOT NULL,
    issuer TEXT NOT NULL,
    approved_at INTEGER NOT NULL,
    revoked_at INTEGER
);
CREATE UNIQUE INDEX IF NOT EXISTS idp_replica_members_installation_member
    ON idp_replica_members (installation_id, member_id);
CREATE UNIQUE INDEX IF NOT EXISTS idp_replica_members_installation_endpoint
    ON idp_replica_members (installation_id, endpoint_id);

CREATE TABLE IF NOT EXISTS idp_replica_signers (
    id UUID PRIMARY KEY,
    member_id UUID NOT NULL,
    active_member_id UUID,
    key_id UUID NOT NULL,
    issuer TEXT NOT NULL,
    public_jwk TEXT NOT NULL,
    approved_at INTEGER NOT NULL,
    revoked_at INTEGER,
    expires_at INTEGER
);
CREATE UNIQUE INDEX IF NOT EXISTS idp_replica_signers_key_id ON idp_replica_signers (key_id);
CREATE INDEX IF NOT EXISTS idp_replica_signers_member
    ON idp_replica_signers (member_id, revoked_at);
CREATE UNIQUE INDEX IF NOT EXISTS idp_replica_signers_active_member
    ON idp_replica_signers (active_member_id);

CREATE TABLE IF NOT EXISTS roles (
    row_id UUID PRIMARY KEY,
    id UUID,
    incarnation INTEGER NOT NULL DEFAULT 0,
    application_id UUID,
    name TEXT,
    description TEXT,
    revoked_at INTEGER,
    created_at INTEGER,
    updated_at INTEGER
);
CREATE UNIQUE INDEX IF NOT EXISTS roles_domain_id ON roles (id, incarnation);
CREATE UNIQUE INDEX IF NOT EXISTS roles_application_name ON roles (application_id, name, incarnation);

CREATE TABLE IF NOT EXISTS permissions (
    row_id UUID PRIMARY KEY,
    id UUID,
    incarnation INTEGER NOT NULL DEFAULT 0,
    application_id UUID,
    name TEXT,
    description TEXT,
    revoked_at INTEGER,
    created_at INTEGER,
    updated_at INTEGER
);
CREATE UNIQUE INDEX IF NOT EXISTS permissions_domain_id ON permissions (id, incarnation);
CREATE UNIQUE INDEX IF NOT EXISTS permissions_application_name ON permissions (application_id, name, incarnation);

CREATE TABLE IF NOT EXISTS rbac_revocations (
    policy_kind TEXT NOT NULL,
    application_id UUID NOT NULL,
    policy_id UUID NOT NULL,
    incarnation INTEGER NOT NULL,
    PRIMARY KEY (policy_kind, application_id, policy_id, incarnation)
);

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

CREATE TABLE IF NOT EXISTS sync_selections (
    device_id UUID NOT NULL,
    resource_id UUID NOT NULL,
    application_id UUID NOT NULL,
    owner_subject TEXT NOT NULL,
    selected_at INTEGER NOT NULL,
    revoked_at INTEGER,
    PRIMARY KEY (device_id, resource_id)
);
CREATE INDEX IF NOT EXISTS sync_selections_resource ON sync_selections (resource_id);
CREATE INDEX IF NOT EXISTS sync_selections_device ON sync_selections (device_id);

CREATE TABLE IF NOT EXISTS local_device_copy_cleanup (device_id TEXT PRIMARY KEY, cleanup_id UUID NOT NULL, completed BOOLEAN NOT NULL);
CREATE TABLE IF NOT EXISTS devices (id UUID PRIMARY KEY, owner_subject TEXT NOT NULL, name TEXT, public_key TEXT, address TEXT, state INTEGER, approved_at INTEGER, revoked_at INTEGER, created_at INTEGER, updated_at INTEGER);
CREATE UNIQUE INDEX IF NOT EXISTS devices_public_key ON devices (public_key);
CREATE TABLE IF NOT EXISTS local_device_enrollments (device_id TEXT PRIMARY KEY, enrollment_code_hash BLOB, enrollment_expires_at INTEGER, pairing_accepting_public_key TEXT);
CREATE TABLE IF NOT EXISTS local_device_join_bindings (request_id TEXT PRIMARY KEY, device_id UUID NOT NULL);
CREATE TABLE IF NOT EXISTS device_selection_policies (device_id UUID NOT NULL, owner_subject TEXT NOT NULL, application_id UUID, selected_kind TEXT, selected_id UUID, admin_allowed BOOLEAN NOT NULL, device_key TEXT PRIMARY KEY);
CREATE UNIQUE INDEX IF NOT EXISTS device_selection_policies_device ON device_selection_policies (device_id);
CREATE TABLE IF NOT EXISTS device_resource_selections (id UUID NOT NULL, device_id UUID NOT NULL, owner_subject TEXT NOT NULL, application_id UUID NOT NULL, selected_kind TEXT NOT NULL, selected_id UUID NOT NULL, selected BOOLEAN NOT NULL, PRIMARY KEY (device_id, owner_subject, application_id, selected_kind, selected_id));
CREATE UNIQUE INDEX IF NOT EXISTS device_resource_selections_id ON device_resource_selections (id);
CREATE TABLE IF NOT EXISTS device_selection_denials (id UUID PRIMARY KEY, selection_id UUID NOT NULL);
CREATE INDEX IF NOT EXISTS device_selection_denials_selection ON device_selection_denials (selection_id);
