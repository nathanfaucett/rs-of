use std::{collections::HashSet, fmt, io};

use axum::Router;

use idp_service::repo::{ClientRepo, KeyRepo, PrivateKeyKeyringRepo, UserRepo};
use idp_service::util::verify_password;
use iroh::EndpointId;
use management_service::LocalPermissions;
use of_model::id::Id;
use of_model::idp::contract::client_registration::ClientRegistration;
use of_model::idp::contract::client_type::ClientType;
use of_model::idp::contract::entity_type::EntityType;
use of_model::idp::contract::grant_type::GrantType;

use idp_server::{
    AppConfig, RouterState, build_state_with_keys, ensure_initial_signer, openapi_router,
    validate_metadata_root,
};
use setup::operator_login_registration;

pub const STORAGE_SYNC_AUDIENCE: &str = "storage:sync";

/// Retained Create identifiers and the operator and service Client inputs.
#[derive(Clone, Copy)]
pub struct NodeIdp<'a> {
    pub user_id: Id,
    pub username: &'a str,
    /// The external OAuth Client ID of the public operator login.
    pub login_id: Id,
    pub member_id: Id,
    /// The initial issuer key ID. Keep this value after issuer key rotation.
    pub key_id: Id,
    pub endpoint_id: EndpointId,
    /// Supply a password to provision. Omit it to verify retained records.
    pub password: Option<&'a str>,
    /// Public Device and Storage API URIs used as operator token audiences.
    pub device_uri: &'a str,
    pub storage_uri: &'a str,
    /// Only this Client can receive the retained Storage introspection audience upgrade.
    pub storage_introspection_client: Id,
    /// Explicit least-privilege, shared-secret Client Credentials registrations.
    pub clients: &'a [ClientRegistration],
}

impl fmt::Debug for NodeIdp<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeIdp").finish_non_exhaustive()
    }
}

/// Builds the unified Create `IdP` in an initialized shared database.
///
/// Returns the router and the internal Application ID of the public operator login.
/// The caller owns policy setup. This does not migrate standalone setup data.
///
/// # Errors
/// Returns an error if configuration, provisioning, or retained identity verification fails.
pub async fn build_node_idp(
    config: &AppConfig,
    database: &db::EmbeddedDatabase,
    spec: NodeIdp<'_>,
) -> io::Result<(Router, Id)> {
    validate_spec(config, &spec)?;
    let login = operator_login_registration(
        spec.login_id,
        &config.api_public_uri,
        spec.device_uri,
        spec.storage_uri,
    );
    let mut config = config.clone();
    let endpoint = spec.endpoint_id.to_string();
    let binding =
        config.retained_node_signer_binding(spec.member_id, spec.key_id, Some(&endpoint))?;
    // Keep the endpoint and initial key bound even when KeyService loads a rotated key.
    config.key_namespace = format!(
        "{}:embedded:{endpoint}:{}",
        config.key_namespace, spec.key_id
    );
    let engine = database.client();
    let storage = database.storage();
    let provision = spec.password.is_some();
    if provision {
        ensure_initial_signer(
            engine.clone(),
            PrivateKeyKeyringRepo::new_local(&storage).map_err(io::Error::other)?,
            &config.key_namespace,
            &config.oauth2.issuer,
            &binding,
        )
        .await?;
    }

    config.oauth2.local_signer = Some(binding);
    let permissions = LocalPermissions::new(engine.clone());
    let keys = PrivateKeyKeyringRepo::new_local(&storage).map_err(io::Error::other)?;
    let state = build_state_with_keys(&config, engine, None, None, Some(permissions), keys).await?;
    ensure_operator(&state, &spec).await?;
    let application = ensure_registration(&state, login, provision, false).await?;
    let storage_client = spec.storage_introspection_client.to_string();
    for registration in spec.clients {
        ensure_registration(
            &state,
            registration.clone(),
            provision,
            registration.client_id.as_deref() == Some(storage_client.as_str()),
        )
        .await?;
    }
    Ok((
        openapi_router(state, config.server.prefix())
            .split_for_parts()
            .0,
        application,
    ))
}

fn validate_spec(config: &AppConfig, spec: &NodeIdp<'_>) -> io::Result<()> {
    config.validate_installation_settings()?;
    config.validate_service_audience()?;
    if config.identity_database.is_some()
        || config.oauth2.trusted_issuer_jwks.is_some()
        || spec.user_id.is_nil()
        || spec.login_id.is_nil()
        || [
            spec.username,
            &config.api_public_uri,
            spec.device_uri,
            spec.storage_uri,
        ]
        .iter()
        .any(|value| value.trim().is_empty())
        || spec
            .password
            .is_some_and(|password| password.trim().is_empty())
    {
        return Err(invalid("invalid embedded IdP configuration"));
    }

    let mut ids = HashSet::from([spec.login_id.to_string()]);
    for client in spec.clients {
        if !client
            .client_id
            .as_deref()
            .is_some_and(|id| !id.trim().is_empty() && ids.insert(id.to_owned()))
            || client.client_type != ClientType::Confidential
            || !client.token_endpoint_auth_method.uses_secret_verifier()
            || client.allowed_grant_types != [GrantType::ClientCredentials]
            || !client.redirect_uris.is_empty()
            || !client.response_types.is_empty()
            || client.allowed_audiences.is_empty()
            || client.allowed_scopes.is_empty()
            || client.application.uri.trim().is_empty()
            || client
                .client_secret
                .as_deref()
                .is_none_or(|secret| secret.trim().is_empty())
        {
            return Err(invalid("invalid embedded service Client registration"));
        }
    }
    Ok(())
}

async fn ensure_operator(state: &RouterState, spec: &NodeIdp<'_>) -> io::Result<()> {
    let oauth = &state.oauth2_service;
    if let Some(password) = spec.password {
        oauth
            .ensure_initial_user(spec.user_id, spec.username, password)
            .await
            .map_err(|error| io::Error::other(format!("embedded IdP User: {error:?}")))?;
    }
    if !oauth
        .user_repo
        .find_user_by_id(spec.user_id)
        .await
        .map_err(io::Error::other)?
        .is_some_and(|user| !user.blocked && user.name == spec.username)
        || oauth
            .user_repo
            .find_user_password_hash(spec.user_id)
            .await
            .map_err(io::Error::other)?
            .is_none_or(|hash| hash.is_empty())
    {
        return Err(invalid(
            "embedded IdP operator or password verifier is missing or changed",
        ));
    }
    let keys = &oauth.key_service;
    let root = match keys
        .key_repo()
        .find_active_entity_root_key(EntityType::User, spec.user_id)
        .await
        .map_err(io::Error::other)?
    {
        Some(root) => root,
        None if spec.password.is_some() => keys
            .create_initial_root_with_capture(
                EntityType::User,
                spec.user_id,
                "Local User".into(),
                |_, _| Ok(()),
            )
            .await
            .map_err(io::Error::other)?,
        None => return Err(invalid("embedded IdP operator root is missing")),
    };
    validate_metadata_root(&root, EntityType::User, spec.user_id)
}

async fn ensure_registration(
    state: &RouterState,
    mut expected: ClientRegistration,
    provision: bool,
    upgrade_storage_sync: bool,
) -> io::Result<Id> {
    let oauth = &state.oauth2_service;
    let id = expected
        .client_id
        .as_deref()
        .expect("validated embedded Client ID");
    let client = match oauth
        .client_repo
        .find_client_by_client_id(id)
        .await
        .map_err(io::Error::other)?
    {
        Some(client) => client,
        None if provision => oauth
            .client_repo
            .create_client(expected.clone())
            .await
            .map_err(io::Error::other)?,
        None => return Err(invalid("embedded IdP Client is missing or inactive")),
    };
    let mut current = oauth
        .get_client(id)
        .await
        .map_err(|error| io::Error::other(format!("embedded IdP Client: {error:?}")))?;

    if let Some(secret) = expected.client_secret.take()
        && !verify_password(&secret, &client.client_secret_hash).map_err(|error| {
            invalid(&format!(
                "invalid embedded IdP Client secret verifier: {error}"
            ))
        })?
    {
        return Err(invalid("embedded IdP Client secret does not match"));
    }
    expected
        .client_uri
        .get_or_insert_with(|| expected.application.uri.clone());
    expected
        .application
        .name
        .get_or_insert_with(|| expected.client_name.clone());
    let upgrade = current != expected;
    if upgrade {
        current.allowed_audiences.push(STORAGE_SYNC_AUDIENCE.into());
        if !upgrade_storage_sync || current != expected {
            return Err(invalid("embedded IdP Client registration changed"));
        }
    }
    let root = oauth
        .key_service
        .key_repo()
        .find_active_entity_root_key(EntityType::Client, client.id)
        .await
        .map_err(io::Error::other)?
        .ok_or_else(|| invalid("embedded IdP Client root is missing"))?;
    validate_metadata_root(&root, EntityType::Client, client.id)?;
    if upgrade {
        let mut client = client;
        client.allowed_audiences = expected.allowed_audiences;
        return oauth
            .client_repo
            .update_client(client, None)
            .await
            .map(|client| client.application_id)
            .map_err(io::Error::other);
    }
    Ok(client.application_id)
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use std::{env, fs, path::Path};

    use axum::{
        Router,
        body::{Body, to_bytes},
        http::Request,
    };
    use db::EmbeddedDatabase;
    use idp_service::setup_credentials::SetupRelationship;
    use iroh::SecretKey;
    use of_model::id::Id;
    use tower::ServiceExt;

    use super::{AppConfig, NodeIdp, build_node_idp};
    use crate::node_runtime::tests::run_on_node_runtime;

    async fn assert_token(router: Router, form: String) {
        let response = router
            .oneshot(
                Request::post("/idp/oauth2/token")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(form))
                    .expect("token request"),
            )
            .await
            .expect("token response");
        let status = response.status();
        let body = to_bytes(response.into_body(), 16 * 1024)
            .await
            .expect("token body");
        assert_eq!(status, 200, "{}", String::from_utf8_lossy(&body));
    }

    #[test]
    fn embedded_node_upgrades_only_the_exact_old_storage_client() {
        run_on_node_runtime(check_retained_storage_upgrade());
    }

    async fn check_retained_storage_upgrade() {
        let dir = env::temp_dir().join(format!("idp-embedded-test-{}", Id::now_v7()));
        let mut config = AppConfig {
            data_dir: dir
                .join("unused-idp-directory")
                .to_string_lossy()
                .into_owned(),
            api_public_uri: "https://node.test/idp".into(),
            ..AppConfig::default()
        };
        config.oauth2.issuer.clone_from(&config.api_public_uri);
        config.server.prefix = Some("/idp".into());
        let service_id = Id::now_v7();
        let device = "https://node.test/device";
        let storage = "https://node.test/storage";
        let mut client = SetupRelationship::StorageIdp.registration(
            service_id,
            &config.api_public_uri,
            device,
            storage,
        );
        client.client_secret = Some("test-service-secret".into());
        let clients = [client];
        let spec = NodeIdp {
            user_id: Id::now_v7(),
            username: "node-operator",
            login_id: Id::now_v7(),
            member_id: Id::now_v7(),
            key_id: Id::now_v7(),
            endpoint_id: SecretKey::generate().public(),
            password: None,
            device_uri: device,
            storage_uri: storage,
            storage_introspection_client: service_id,
            clients: &clients,
        };
        let database = EmbeddedDatabase::open(&dir)
            .await
            .expect("embedded database");
        let application = provision_legacy(&config, &database, spec).await;
        drop(database);

        let database = EmbeddedDatabase::open(&dir)
            .await
            .expect("reopen embedded database");
        let mut updated = clients[0].clone();
        updated.allowed_audiences.push("storage:sync".into());
        let mut changed = updated.clone();
        changed.allowed_scopes.push("idp.user.lookup".into());
        build_node_idp(
            &config,
            &database,
            NodeIdp {
                clients: &[changed],
                ..spec
            },
        )
        .await
        .expect_err("an unrelated registration change must not be repaired");
        let updated_clients = [updated];
        let current = NodeIdp {
            clients: &updated_clients,
            ..spec
        };
        let (router, reopened) = build_node_idp(&config, &database, current)
            .await
            .expect("upgrade exact old Storage registration without a password");
        build_node_idp(&config, &database, spec)
            .await
            .expect_err("the old registration must no longer match after the upgrade");
        assert_eq!(reopened, application);
        assert_token(router, format!("grant_type=client_credentials&client_id={service_id}&client_secret=test-service-secret&scope=idp.token.validate&audience=https%3A%2F%2Fnode.test%2Fidp")).await;
        for changed in [
            NodeIdp {
                endpoint_id: SecretKey::generate().public(),
                ..current
            },
            NodeIdp {
                login_id: Id::now_v7(),
                ..current
            },
            NodeIdp {
                user_id: Id::now_v7(),
                ..current
            },
            NodeIdp {
                key_id: Id::now_v7(),
                ..current
            },
        ] {
            build_node_idp(&config, &database, changed)
                .await
                .expect_err("reopen must reject changed identity");
        }
        let (_, retained) = build_node_idp(&config, &database, current)
            .await
            .expect("failed reopens must leave the original IdP intact");
        assert_eq!(retained, application);
        assert!(!Path::new(&config.data_dir).exists());
        drop(database);
        fs::remove_dir_all(dir).expect("remove test database");
    }

    async fn provision_legacy(
        config: &AppConfig,
        database: &EmbeddedDatabase,
        spec: NodeIdp<'_>,
    ) -> Id {
        build_node_idp(config, database, spec)
            .await
            .expect_err("reopen cannot provision a missing signer");
        let (router, application) = build_node_idp(
            config,
            database,
            NodeIdp {
                password: Some("test-operator-password"),
                ..spec
            },
        )
        .await
        .expect("provision embedded IdP");
        assert!(!application.is_nil());
        assert_token(router, format!("grant_type=password&client_id={}&username=node-operator&password=test-operator-password&scope=openid", spec.login_id)).await;
        application
    }
}
