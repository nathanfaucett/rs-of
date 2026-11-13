use std::io;

use idp_service::repo::{ClientRepo, KeyRepo, UserRepo};
use management_service::{
    INITIAL_ADMINISTRATOR_ACTIONS, ManagementService,
    replica::{DbPermissionRepo, DbRoleRepo},
};
use of_model::idp::{
    contract::{
        ApplicationRegistration, ClientProfile, ClientRegistration, ClientType, EntityType,
        GrantType, IdentityAction, ResponseType, TokenEndpointAuthMethod,
    },
    model::Id,
};

use crate::RouterState;

pub async fn signer(
    client: db::Client,
    storage: &db::SharedDatabase,
    namespace: &str,
    issuer: &str,
) -> io::Result<idp_service::oauth2::NodeSignerBinding> {
    use idp_service::{
        replica::{DbKeyRepo, DbReplicaSignerRepo},
        repo::{KeyService, PrivateKeyKeyringRepo},
    };
    use of_model::idp::contract::{IdpSignerRecord, ReplicaMembership};
    use sha2::{Digest, Sha256};
    let member_id = Id::new_v5(&Id::NAMESPACE_URL, namespace.as_bytes());
    let binding = idp_service::oauth2::NodeSignerBinding {
        member_id,
        key_id: Id::new_v5(&member_id, b"local-signer"),
        endpoint_id: iroh::SecretKey::from_bytes(&Sha256::digest(namespace.as_bytes()).into())
            .public()
            .to_string(),
    };
    let keys = KeyService::new(
        DbKeyRepo::new(client.clone()),
        PrivateKeyKeyringRepo::new_local(storage).map_err(io::Error::other)?,
        namespace,
    );
    let public = keys
        .prepare_node_signer(&binding)
        .map_err(io::Error::other)?;
    DbReplicaSignerRepo::new(client)
        .enroll(
            &ReplicaMembership {
                installation_id: issuer.into(),
                member_id,
                endpoint_id: binding.endpoint_id.clone(),
                issuer: issuer.into(),
                approved_at: 1,
                revoked_at: None,
            },
            &IdpSignerRecord {
                member_id,
                key_id: binding.key_id,
                issuer: issuer.into(),
                public_jwk: public,
                approved_at: Some(1),
                revoked_at: None,
                expires_at: None,
            },
        )
        .await
        .map_err(io::Error::other)?;
    Ok(binding)
}

pub async fn seed(state: &RouterState, origin: &str) -> io::Result<()> {
    let oauth = &state.oauth2_service;
    let user = Id::new_v5(&Id::NAMESPACE_URL, b"of:local:admin");
    seed_user(state, user, "admin@local.test", "password").await?;

    oauth
        .user_repo
        .upsert_primary_user_email(user, "admin@local.test", true)
        .await
        .map_err(io::Error::other)?;

    ManagementService::new(
        DbPermissionRepo::new(state.engine.clone()),
        DbRoleRepo::new(state.engine.clone()),
    )
    .provision_initial_administrator(
        user,
        Id::new_v5(&user, b"administrator"),
        &INITIAL_ADMINISTRATOR_ACTIONS
            .iter()
            .chain([IdentityAction::DevicesEnroll].iter())
            .map(|action| (Id::new_v5(&user, format!("{action:?}").as_bytes()), *action))
            .collect::<Vec<_>>(),
    )
    .await
    .map_err(io::Error::other)?;

    let mut frontend = reqwest::Url::parse(origin).map_err(io::Error::other)?;
    if !matches!(frontend.scheme(), "http" | "https")
        || frontend.host_str().is_none()
        || !frontend.username().is_empty()
        || frontend.password().is_some()
        || frontend.query().is_some()
        || frontend.fragment().is_some()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "local frontend origin must be an HTTP origin",
        ));
    }
    frontend.set_path("/callback");
    let app_redirect = frontend.to_string();
    frontend
        .set_port(Some(5174))
        .map_err(|_| io::Error::other("frontend origin cannot use a port"))?;
    let password_redirect = frontend.to_string();
    let audiences = vec![
        state.api_base_uri.clone(),
        state.api_base_uri.trim_end_matches("/idp").to_owned() + "/device",
        state.api_base_uri.trim_end_matches("/idp").to_owned() + "/storage",
        management_service::MANAGEMENT_APPLICATION_URI.into(),
        "storage:databases".into(),
        "storage:filesystems".into(),
    ];
    for (id, redirect) in [
        ("app", Some(app_redirect.clone())),
        ("local-login", Some(app_redirect)),
        ("password-manager", Some(password_redirect)),
        ("local-device", None),
        ("local-management", None),
        ("local-storage", None),
    ] {
        if oauth
            .client_repo
            .find_client_by_client_id(id)
            .await
            .map_err(io::Error::other)?
            .is_some()
        {
            continue;
        }
        let public = redirect.is_some();
        let registration = ClientRegistration {
            application: ApplicationRegistration {
                name: Some(id.into()),
                uri: if public {
                    format!("{}/{}", origin.trim_end_matches('/'), id)
                } else {
                    state.api_base_uri.clone()
                },
                description: None,
            },
            client_id: Some(id.into()),
            client_secret: (!public).then(|| "local-password".into()),
            client_id_issued_at: None,
            client_secret_expires_at: None,
            client_name: id.into(),
            client_uri: None,
            logo_uri: None,
            contacts: vec![],
            terms_of_service_uri: None,
            policy_uri: None,
            client_type: if public {
                ClientType::Public
            } else {
                ClientType::Confidential
            },
            profile: if public {
                ClientProfile::UserAgentBased
            } else {
                ClientProfile::Web
            },
            redirect_uris: redirect.into_iter().collect(),
            allowed_grant_types: if id == "local-login" {
                vec![
                    GrantType::Password,
                    GrantType::RefreshToken,
                    GrantType::TokenExchange,
                ]
            } else if public {
                vec![
                    GrantType::AuthorizationCode,
                    GrantType::RefreshToken,
                    GrantType::TokenExchange,
                ]
            } else {
                vec![GrantType::ClientCredentials]
            },
            response_types: if public {
                vec![ResponseType::Code]
            } else {
                vec![]
            },
            allowed_scopes: if public {
                vec![
                    "openid".into(),
                    "profile".into(),
                    "email".into(),
                    "offline_access".into(),
                    "storage".into(),
                ]
            } else {
                vec!["idp.token.validate".into(), "idp.user.lookup".into()]
            },
            allowed_audiences: audiences.clone(),
            token_endpoint_auth_method: if public {
                TokenEndpointAuthMethod::None
            } else {
                TokenEndpointAuthMethod::ClientSecretPost
            },
            software_statement: None,
            software_id: None,
            software_version: None,
        };

        oauth
            .client_repo
            .create_client(registration)
            .await
            .map_err(io::Error::other)?;
    }
    Ok(())
}

pub async fn seed_user(
    state: &RouterState,
    user: Id,
    username: &str,
    password: &str,
) -> io::Result<()> {
    let oauth = &state.oauth2_service;
    oauth
        .ensure_initial_user(user, Id::new_v5(&user, b"password"), username, password)
        .await
        .map_err(|error| io::Error::other(format!("local user: {error:?}")))?;
    if oauth
        .key_service
        .key_repo()
        .find_by_entity_type_and_id(EntityType::User, user)
        .await
        .map_err(io::Error::other)?
        .is_none()
    {
        oauth
            .key_service
            .ensure_entity_master_key(EntityType::User, user, password)
            .map_err(io::Error::other)?;
        oauth
            .key_service
            .create_key(None, EntityType::User, user, true, username.into(), None)
            .await
            .map_err(io::Error::other)?;
    }
    Ok(())
}
