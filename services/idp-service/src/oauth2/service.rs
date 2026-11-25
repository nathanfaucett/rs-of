use core::fmt;

#[cfg(not(feature = "std"))]
use alloc::{
    borrow::ToOwned,
    boxed::Box,
    format,
    string::{String, ToString},
    sync::Arc,
    vec,
    vec::Vec,
};

use of_model::contract::{
    access_token::AccessToken, authorization_details::AuthorizationDetail, id_token::IdToken,
    principal_type::PrincipalType, refresh_token::RefreshToken, standard_claims::StandardClaims,
    token_response::TokenResponse, token_type::TokenType, token_use::TokenUse,
};
#[cfg(feature = "std")]
use std::{
    env,
    io::{self, Write},
    process,
    sync::Arc,
    thread,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use chrono::{DateTime, Duration, Utc};
use of_model::id::Id;
use of_model::idp::contract::permission_evaluation::{
    MANAGEMENT_APPLICATION_URI, MANAGEMENT_POLICY_ADMINISTRATION_SCOPE,
};
use of_model::idp::model::application::Application;
use of_model::idp::model::client::Client;
use of_model::idp::model::key::Key;
use of_model::idp::{
    contract::{
        application_registration::ApplicationRegistration,
        approve_for_user_request::ApproveForUserRequest,
        authorization_code_grant_request::AuthorizationCodeGrantRequest,
        authorization_code_response::AuthorizationCodeResponse,
        authorization_request::AuthorizationRequest,
        authorization_server_metadata::AuthorizationServerMetadata,
        client_consent_metadata::ClientConsentMetadata,
        client_credentials_grant_request::ClientCredentialsGrantRequest,
        client_registration::ClientRegistration,
        client_type::ClientType,
        device_authorization::DeviceAuthorization,
        device_authorization_request::DeviceAuthorizationRequest,
        entity_type::EntityType,
        error_code::ErrorCode,
        error_response::{ErrorResponse, ErrorResponseResult},
        grant_type::GrantType,
        id_token_claims::IdTokenClaims,
        idp_signer::TokenPrincipalBinding,
        is_allowed_for_user_request::IsAllowedForUserRequest,
        is_allowed_for_user_response::IsAllowedForUserResponse,
        jwk_private::JwkPrivate,
        jwk_public::JwkPublic,
        jwks::Jwks,
        oauth2_client_auth::OAuth2ClientAuth,
        password_grant_request::PasswordGrantRequest,
        refresh_token_grant_request::RefreshTokenGrantRequest,
        revocation_request::RevocationRequest,
        sex::Sex,
        subject_token_type::SubjectTokenType,
        token_endpoint_auth_method::TokenEndpointAuthMethod,
        token_exchange_grant_request::TokenExchangeGrantRequest,
        token_request::TokenRequest,
        user_info::UserInfo,
    },
    model::{oauth2_user_consent::OAuth2UserConsent, user::User},
};
use serde::de::IgnoredAny;

use crate::{
    oauth2::{
        jwt::{decode_jwt, encode_jwt, verify_jwt},
        principal::{ClientPrincipal, Principal, UserPrincipal},
    },
    password_config::PasswordConfig,
    repo::{
        ApplicationRepo, ClientRepo, KeyRepo, KeyService, OAuth2AuthorizationCodeRepo,
        OAuth2RefreshToken, OAuth2RefreshTokenRepo, OAuth2UserConsentRepo, PrivateKeyRepo,
        UserRepo,
    },
    util::{encrypt_password, generate_random_string, verify_password},
};

use crate::oauth2::config::OAuth2Config;

use crate::oauth2::scope::intersect_scopes;

use crate::oauth2::scope::parse_scopes;

use crate::oauth2::authorization::resolve_redirect_uri;

use crate::oauth2::token::validate_authorization_code_grant;

use crate::oauth2::authorization::validate_authorization_details;

use crate::oauth2::authorization::validate_authorization_request;

use crate::oauth2::authorization::validate_dynamic_client_grants;

use crate::oauth2::scope::validate_scopes;

use crate::oauth2::pkce::verify_code_challenge;

#[cfg(all(test, feature = "ofdb"))]
#[path = "refresh_tests.rs"]
mod refresh_tests;

pub struct OAuth2Service<A, C, AC, RT, U, G, K, P> {
    pub application_repo: A,
    pub client_repo: C,
    pub authorization_code_repo: AC,
    pub refresh_token_repo: RT,
    pub user_repo: U,
    pub oauth2_user_consent_repo: G,
    pub key_service: Arc<KeyService<K, P>>,
    pub oauth_config: OAuth2Config,
}

impl<A, C, AC, RT, U, G, K, P> fmt::Debug for OAuth2Service<A, C, AC, RT, U, G, K, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OAuth2Service").finish_non_exhaustive()
    }
}

use of_model::oauth_user::CreateUserRequest;

use of_model::oauth_user::UpdateUserInfoRequest;

#[allow(
    clippy::future_not_send,
    reason = "repository futures intentionally support single-threaded no_std executors"
)]
impl<A, C, AC, RT, U, G, K, P> OAuth2Service<A, C, AC, RT, U, G, K, P>
where
    A: ApplicationRepo,
    C: ClientRepo,
    AC: OAuth2AuthorizationCodeRepo,
    RT: OAuth2RefreshTokenRepo,
    U: UserRepo,
    G: OAuth2UserConsentRepo,
    K: KeyRepo,
    P: PrivateKeyRepo,
{
    pub const fn new(
        application_repo: A,
        client_repo: C,
        authorization_code_repo: AC,
        refresh_token_repo: RT,
        user_repo: U,
        oauth2_user_consent_repo: G,
        key_service: Arc<KeyService<K, P>>,
        oauth_config: OAuth2Config,
    ) -> Self {
        Self {
            application_repo,
            client_repo,
            authorization_code_repo,
            refresh_token_repo,
            user_repo,
            oauth2_user_consent_repo,
            key_service,
            oauth_config,
        }
    }

    /// # Errors
    /// Returns an OAuth error response if the initial User data or credentials are invalid, or persistence fails.
    pub async fn ensure_initial_user(
        &self,
        user_id: Id,
        name: &str,
        password: &str,
    ) -> ErrorResponseResult<User> {
        if user_id == Id::nil() || name.trim().is_empty() {
            return Err(ErrorResponse::new(ErrorCode::InvalidRequest)
                .with_description("a stable User ID and name are required"));
        }
        if password.trim().is_empty() {
            return Err(ErrorResponse::new(ErrorCode::InvalidRequest)
                .with_description("password is required"));
        }

        if let Some(user) = self
            .user_repo
            .find_user_by_id(user_id)
            .await
            .map_err(ErrorResponse::from)?
        {
            if user.name != name {
                return Err(ErrorResponse::new(ErrorCode::InvalidRequest)
                    .with_description("initial user ID is already used by different data"));
            }
            let stored = self
                .user_repo
                .find_user_password_hash(user_id)
                .await
                .map_err(ErrorResponse::from)?
                .ok_or_else(|| {
                    ErrorResponse::new(ErrorCode::InvalidRequest)
                        .with_description("initial User has no local password verifier")
                })?;
            if !verify_password(password, &stored).map_err(|error| {
                ErrorResponse::new(ErrorCode::ServerError).with_description(error.to_string())
            })? {
                return Err(ErrorResponse::new(ErrorCode::InvalidRequest)
                    .with_description("initial user ID is already used by different credentials"));
            }
            return Ok(user);
        }

        self.user_repo
            .create_user_with_password_and_id(user_id, name, password)
            .await
            .map_err(ErrorResponse::from)
    }

    /// # Errors
    /// Returns an OAuth error response if registration validation or persistence fails.
    pub async fn register_client(
        &self,
        request: ClientRegistration,
    ) -> ErrorResponseResult<ClientRegistration> {
        Self::validate_dynamic_client_registration(&request)?;
        self.register_owned_client(request, true).await
    }

    /// # Errors
    /// Returns an OAuth error response if public Client registration is invalid or cannot be persisted.
    pub async fn register_public_client(
        &self,
        request: ClientRegistration,
        allow_application_creation: bool,
    ) -> ErrorResponseResult<ClientRegistration> {
        if request.client_id.is_some()
            || request.client_secret.is_some()
            || request.client_type != ClientType::Public
            || request.token_endpoint_auth_method != TokenEndpointAuthMethod::None
            || request.application.uri.trim().is_empty()
            || !request
                .allowed_grant_types
                .contains(&GrantType::AuthorizationCode)
        {
            return Err(ErrorResponse::new(ErrorCode::InvalidRequest));
        }
        Self::validate_dynamic_client_registration(&request)?;
        self.register_owned_client(request, allow_application_creation)
            .await
    }

    /// # Errors
    /// Returns an OAuth error response if infrastructure Client registration is invalid or cannot be persisted.
    pub async fn register_infrastructure_client(
        &self,
        request: ClientRegistration,
    ) -> ErrorResponseResult<ClientRegistration> {
        if request.client_type != ClientType::Confidential
            || !request.token_endpoint_auth_method.uses_secret_verifier()
            || request.allowed_grant_types != [GrantType::ClientCredentials]
        {
            return Err(ErrorResponse::new(ErrorCode::InvalidRequest));
        }
        self.register_owned_client(request, true).await
    }

    /// # Errors
    /// Returns an OAuth error response if existing infrastructure Client data differs or persistence fails.
    pub async fn ensure_infrastructure_client(
        &self,
        request: ClientRegistration,
    ) -> ErrorResponseResult<ClientRegistration> {
        if request.client_type != ClientType::Confidential
            || !request.token_endpoint_auth_method.uses_secret_verifier()
            || request.allowed_grant_types != [GrantType::ClientCredentials]
        {
            return Err(ErrorResponse::new(ErrorCode::InvalidRequest));
        }
        let client_id = request
            .client_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::InvalidRequest)
                    .with_description("stable infrastructure client ID is required")
            })?
            .to_owned();
        let client_secret = request
            .client_secret
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::InvalidRequest)
                    .with_description("infrastructure client secret is required")
            })?
            .to_owned();

        let Some(client) = self
            .client_repo
            .find_client_by_client_id(&client_id)
            .await
            .map_err(ErrorResponse::from)?
        else {
            return self.register_infrastructure_client(request).await;
        };

        let application = self
            .application_repo
            .find_by_id(client.application_id)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::InvalidRequest)
                    .with_description("infrastructure client application is missing")
            })?;
        let secret_matches =
            verify_password(&client_secret, &client.client_secret_hash).map_err(|error| {
                ErrorResponse::new(ErrorCode::ServerError).with_description(error.to_string())
            })?;
        let application_name = request
            .application
            .name
            .as_deref()
            .unwrap_or(&request.client_name);
        let client_uri = request
            .client_uri
            .as_deref()
            .unwrap_or(&request.application.uri);
        let exact_match = secret_matches
            && application.name == application_name
            && application.uri == request.application.uri
            && application.description == request.application.description
            && client.client_name == request.client_name
            && client.client_uri == client_uri
            && client.client_id_issued_at.map(|time| time.timestamp())
                == request.client_id_issued_at
            && client.client_secret_expires_at.map(|time| time.timestamp())
                == request.client_secret_expires_at
            && client.client_type == request.client_type
            && client.profile == request.profile
            && client.token_endpoint_auth_method == request.token_endpoint_auth_method
            && client.allowed_grant_types == request.allowed_grant_types
            && client.response_types == request.response_types
            && client.allowed_scopes == request.allowed_scopes
            && client.allowed_audiences == request.allowed_audiences
            && client.redirect_uris == request.redirect_uris
            && client.logo_uri == request.logo_uri
            && client.contacts == request.contacts
            && client.terms_of_service_uri == request.terms_of_service_uri
            && client.policy_uri == request.policy_uri
            && client.software_statement == request.software_statement
            && client.software_id == request.software_id
            && client.software_version == request.software_version;
        if !exact_match {
            return Err(ErrorResponse::new(ErrorCode::InvalidRequest)
                .with_description("infrastructure client ID is already used by different data"));
        }

        let mut registration: ClientRegistration = client.into();
        registration.application = request.application;
        registration.client_secret = Some(client_secret);
        Ok(registration)
    }

    fn validate_dynamic_client_registration(
        request: &ClientRegistration,
    ) -> ErrorResponseResult<()> {
        if request
            .allowed_scopes
            .iter()
            .any(|scope| scope == MANAGEMENT_POLICY_ADMINISTRATION_SCOPE)
        {
            return Err(ErrorResponse::new(ErrorCode::InvalidScope)
                .with_description("this scope requires trusted operator provisioning"));
        }
        validate_dynamic_client_grants(&request.allowed_grant_types)
    }

    async fn register_owned_client(
        &self,
        request: ClientRegistration,
        allow_application_creation: bool,
    ) -> ErrorResponseResult<ClientRegistration> {
        let client = ClientRegistration {
            client_id: Some(
                request
                    .client_id
                    .unwrap_or_else(generate_random_string::<32>),
            ),
            client_secret: (request.client_type == ClientType::Confidential
                && request.token_endpoint_auth_method.uses_secret_verifier())
            .then(|| {
                request
                    .client_secret
                    .unwrap_or_else(generate_random_string::<32>)
            }),
            ..request
        };

        let issued_secret = client.client_secret.clone();
        let client = if allow_application_creation {
            self.client_repo.create_client(client).await
        } else {
            self.client_repo
                .create_client_in_existing_application(client)
                .await
        }
        .map_err(ErrorResponse::from)?;

        let mut registration = self.registration_for_client(client).await?;
        registration.client_secret = issued_secret;
        Ok(registration)
    }

    async fn registration_for_client(
        &self,
        client: Client,
    ) -> ErrorResponseResult<ClientRegistration> {
        let application = self.get_application(client.application_id).await?;
        let mut registration: ClientRegistration = client.into();
        registration.application = ApplicationRegistration {
            name: Some(application.name),
            uri: application.uri,
            description: application.description,
        };
        Ok(registration)
    }

    /// # Errors
    /// Returns an OAuth error response if the Client is absent or lookup fails.
    pub async fn get_client(&self, client_id: &str) -> ErrorResponseResult<ClientRegistration> {
        let client = self
            .client_repo
            .find_client_by_client_id(client_id)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::InvalidClient).with_description("client not found")
            })?;
        self.registration_for_client(client).await
    }

    /// # Errors
    /// Returns an OAuth error response if public consent metadata is unavailable or lookup fails.
    pub async fn get_client_consent_metadata(
        &self,
        client_id: &str,
    ) -> ErrorResponseResult<ClientConsentMetadata> {
        let client = self
            .client_repo
            .find_client_registration(client_id)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| ErrorResponse::new(ErrorCode::NotFound))?;
        if client.client_type != ClientType::Public
            || !client
                .allowed_grant_types
                .contains(&GrantType::AuthorizationCode)
            || client
                .allowed_grant_types
                .contains(&GrantType::ClientCredentials)
            || client
                .allowed_scopes
                .iter()
                .any(|scope| scope.starts_with("idp.") || scope.starts_with("management."))
        {
            return Err(ErrorResponse::new(ErrorCode::NotFound));
        }
        let application = self.get_application(client.application_id).await?;
        Ok(ClientConsentMetadata {
            application: ApplicationRegistration {
                name: Some(application.name),
                uri: application.uri,
                description: application.description,
            },
            client_id: client.client_id,
            client_name: client.client_name,
            client_uri: Some(client.client_uri),
            logo_uri: client.logo_uri,
            terms_of_service_uri: client.terms_of_service_uri,
            policy_uri: client.policy_uri,
            redirect_uris: client.redirect_uris,
            allowed_scopes: client.allowed_scopes,
        })
    }

    /// # Errors
    /// Returns an OAuth error response if Applications cannot be listed.
    pub async fn list_applications(
        &self,
        offset: u32,
        limit: u32,
    ) -> ErrorResponseResult<Vec<Application>> {
        self.application_repo
            .list_applications(offset, limit.min(100))
            .await
            .map_err(ErrorResponse::from)
    }

    /// # Errors
    /// Returns an OAuth error response if the Application is absent or lookup fails.
    pub async fn get_application(&self, id: Id) -> ErrorResponseResult<Application> {
        self.application_repo
            .find_by_id(id)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| ErrorResponse::new(ErrorCode::NotFound))
    }

    /// # Errors
    /// Returns an OAuth error response if Application creation fails.
    pub async fn create_application(
        &self,
        name: String,
        uri: String,
        description: Option<String>,
    ) -> ErrorResponseResult<Application> {
        self.application_repo
            .create_application(name, uri, description)
            .await
            .map_err(ErrorResponse::from)
    }

    /// # Errors
    /// Returns an OAuth error response if Application update fails.
    pub async fn update_application(
        &self,
        application: Application,
    ) -> ErrorResponseResult<Application> {
        self.application_repo
            .update_application(application)
            .await
            .map_err(ErrorResponse::from)
    }

    /// # Errors
    /// Returns an OAuth error response if Application deletion fails.
    pub async fn delete_application(&self, id: Id) -> ErrorResponseResult<()> {
        self.application_repo
            .delete_application_by_id(id)
            .await
            .map_err(ErrorResponse::from)
    }

    /// # Errors
    /// Returns an OAuth error response if the Application URI is unknown or lookup fails.
    pub async fn application_id_for_uri(&self, uri: &str) -> ErrorResponseResult<Id> {
        self.application_repo
            .find_by_uri(uri)
            .await
            .map_err(ErrorResponse::from)?
            .map(|application| application.id)
            .ok_or_else(|| ErrorResponse::new(ErrorCode::NotFound))
    }

    fn validate_bearer_registration(
        claims: &StandardClaims,
        client: &Client,
    ) -> ErrorResponseResult<Id> {
        if claims
            .scope
            .iter()
            .any(|scope| !client.allowed_scopes.contains(scope))
            || (claims.principal_type == PrincipalType::Client
                && (claims.sub != client.id.to_string()
                    || !client
                        .allowed_grant_types
                        .contains(&GrantType::ClientCredentials)
                    || !client.allowed_audiences.contains(&claims.aud)))
        {
            return Err(ErrorResponse::new(ErrorCode::NotAuthorized));
        }
        Ok(client.application_id)
    }

    /// # Errors
    /// Returns an OAuth error response if the Client is unknown or lookup fails.
    pub async fn application_id_for_client(&self, client_id: &str) -> ErrorResponseResult<Id> {
        self.client_repo
            .find_client_registration(client_id)
            .await
            .map_err(ErrorResponse::from)?
            .map(|client| client.application_id)
            .ok_or_else(|| ErrorResponse::new(ErrorCode::InvalidClient))
    }

    /// # Errors
    /// Returns an OAuth error response if Clients cannot be listed.
    pub async fn list_clients(
        &self,
        offset: u32,
        limit: u32,
    ) -> ErrorResponseResult<Vec<ClientRegistration>> {
        let clients = self
            .client_repo
            .list_clients(offset, limit)
            .await
            .map_err(ErrorResponse::from)?;

        Ok(clients.into_iter().map(Into::into).collect())
    }

    /// # Errors
    /// Returns an OAuth error response if Client validation or update fails.
    pub async fn update_client(
        &self,
        client_id: &str,
        request: ClientRegistration,
    ) -> ErrorResponseResult<ClientRegistration> {
        Self::validate_dynamic_client_registration(&request)?;
        self.update_owned_client(client_id, request).await
    }

    /// # Errors
    /// Returns an OAuth error response if infrastructure Client validation or update fails.
    pub async fn update_infrastructure_client(
        &self,
        client_id: &str,
        request: ClientRegistration,
    ) -> ErrorResponseResult<ClientRegistration> {
        if request.client_type != ClientType::Confidential
            || !request.token_endpoint_auth_method.uses_secret_verifier()
            || request.allowed_grant_types != [GrantType::ClientCredentials]
        {
            return Err(ErrorResponse::new(ErrorCode::InvalidRequest));
        }
        self.update_owned_client(client_id, request).await
    }

    async fn update_owned_client(
        &self,
        client_id: &str,
        request: ClientRegistration,
    ) -> ErrorResponseResult<ClientRegistration> {
        let existing = self
            .client_repo
            .find_client_registration(client_id)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::InvalidClient).with_description("client not found")
            })?;

        let uses_verifier = request.client_type == ClientType::Confidential
            && request.token_endpoint_auth_method.uses_secret_verifier();
        let issued_secret = uses_verifier.then_some(request.client_secret).flatten();
        let new_secret_hash = issued_secret
            .as_deref()
            .map(|secret| {
                if secret.trim().is_empty() {
                    return Err(ErrorResponse::new(ErrorCode::InvalidRequest)
                        .with_description("a confidential client requires a nonempty secret"));
                }
                encrypt_password(&PasswordConfig::default(), secret).map_err(|error| {
                    ErrorResponse::new(ErrorCode::ServerError).with_description(error.to_string())
                })
            })
            .transpose()?;
        let now = Utc::now();
        let client = Client {
            id: existing.id,
            application_id: existing.application_id,
            client_id: existing.client_id,
            client_secret_hash: String::new(),
            client_id_issued_at: existing.client_id_issued_at,
            client_secret_expires_at: request
                .client_secret_expires_at
                .and_then(|ts| DateTime::from_timestamp(ts, 0))
                .or(existing.client_secret_expires_at),
            client_name: request.client_name,
            client_uri: request.client_uri.unwrap_or(existing.client_uri),
            redirect_uris: request.redirect_uris,
            client_type: request.client_type,
            profile: request.profile,
            token_endpoint_auth_method: request.token_endpoint_auth_method,
            allowed_grant_types: request.allowed_grant_types,
            response_types: request.response_types,
            allowed_scopes: request.allowed_scopes,
            allowed_audiences: request.allowed_audiences,
            logo_uri: request.logo_uri,
            contacts: request.contacts,
            terms_of_service_uri: request.terms_of_service_uri,
            policy_uri: request.policy_uri,
            software_statement: request.software_statement,
            software_id: request.software_id,
            software_version: request.software_version,
            created_at: existing.created_at,
            updated_at: now,
        };

        let client = self
            .client_repo
            .update_client(client, new_secret_hash)
            .await
            .map_err(ErrorResponse::from)?;
        let mut registration = self.registration_for_client(client).await?;
        registration.client_secret = issued_secret;
        Ok(registration)
    }

    /// # Errors
    /// Returns an OAuth error response if Client deletion fails.
    pub async fn delete_client(&self, client_id: &str) -> ErrorResponseResult<()> {
        self.client_repo
            .delete_client_by_client_id(client_id)
            .await
            .map_err(ErrorResponse::from)
    }

    /// # Errors
    /// Returns an OAuth error response if authorization, consent, or code creation fails.
    pub async fn authorize<PT: Principal + ?Sized>(
        &self,
        request: AuthorizationRequest,
        principal: &PT,
    ) -> ErrorResponseResult<AuthorizationCodeResponse> {
        let client = self
            .client_repo
            .find_client_by_client_id(&request.client_id)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::InvalidClient).with_description("client not found")
            })?;

        validate_authorization_request(&request, &client, self.oauth_config.require_pkce)?;
        let redirect_uri = resolve_redirect_uri(&request, &client)?;
        let requested_scopes = request.scope.as_deref().map_or_default(parse_scopes);
        let scopes = intersect_scopes(&requested_scopes, &client.allowed_scopes);
        let normalized_scope = Self::normalize_scopes(&scopes);

        if principal.get_entity_type() != EntityType::User {
            return Err(ErrorResponse::new(ErrorCode::AccessDenied)
                .with_description("only users can authorize clients"));
        }

        let consent = self
            .oauth2_user_consent_repo
            .find_user_consent(
                principal.get_entity_id(),
                &client.client_id,
                &redirect_uri,
                &normalized_scope,
            )
            .await
            .map_err(ErrorResponse::from)?;

        if consent.is_none() {
            return Err(ErrorResponse::new(ErrorCode::AccessDenied)
                .with_description("client approval required"));
        }

        let authorization_code = self
            .authorization_code_repo
            .create_authorization_code(
                client.client_id,
                principal.get_key().id,
                redirect_uri,
                scopes,
                request.resource,
                request.code_challenge,
                request.code_challenge_method,
                request.nonce,
                checked_expiration(Utc::now(), self.oauth_config.authorization_code_ttl_secs)?,
            )
            .await
            .map_err(ErrorResponse::from)?;

        Ok(AuthorizationCodeResponse::Success {
            code: authorization_code.code,
            state: request.state,
            issuer: Some(self.oauth_config.issuer.clone()),
        })
    }

    /// # Errors
    /// Returns an OAuth error response if approval validation or persistence fails.
    pub async fn approve_for_user<PT: Principal + ?Sized>(
        &self,
        request: ApproveForUserRequest,
        principal: &PT,
    ) -> ErrorResponseResult<IsAllowedForUserResponse> {
        if principal.get_entity_type() != EntityType::User {
            return Err(ErrorResponse::new(ErrorCode::AccessDenied)
                .with_description("only users can approve clients"));
        }

        let client = self
            .client_repo
            .find_client_by_client_id(&request.client_id)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::InvalidClient).with_description("client not found")
            })?;

        if !client.redirect_uris.contains(&request.redirect_uri) {
            return Err(ErrorResponse::new(ErrorCode::InvalidRequest)
                .with_description("redirect_uri is not allowed for this client"));
        }

        let requested_scopes = parse_scopes(&request.scope);
        validate_scopes(&requested_scopes, &client.allowed_scopes)?;
        let effective_scopes = intersect_scopes(&requested_scopes, &client.allowed_scopes);
        let normalized_scope = Self::normalize_scopes(&effective_scopes);

        self.oauth2_user_consent_repo
            .upsert_user_consent(
                principal.get_entity_id(),
                &client.client_id,
                &request.redirect_uri,
                &normalized_scope,
            )
            .await
            .map_err(ErrorResponse::from)?;

        Ok(IsAllowedForUserResponse { allowed: true })
    }

    /// # Errors
    /// Returns an OAuth error response if consent validation or lookup fails.
    pub async fn is_allowed_for_user<PT: Principal + ?Sized>(
        &self,
        request: IsAllowedForUserRequest,
        principal: &PT,
    ) -> ErrorResponseResult<IsAllowedForUserResponse> {
        if principal.get_entity_type() != EntityType::User {
            return Err(ErrorResponse::new(ErrorCode::AccessDenied)
                .with_description("only users can check client approval"));
        }

        let client = self
            .client_repo
            .find_client_by_client_id(&request.client_id)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::InvalidClient).with_description("client not found")
            })?;

        if !client.redirect_uris.contains(&request.redirect_uri) {
            return Ok(IsAllowedForUserResponse { allowed: false });
        }

        let requested_scopes = parse_scopes(&request.scope);
        validate_scopes(&requested_scopes, &client.allowed_scopes)?;
        let effective_scopes = intersect_scopes(&requested_scopes, &client.allowed_scopes);
        let normalized_scope = Self::normalize_scopes(&effective_scopes);

        let consent = self
            .oauth2_user_consent_repo
            .find_user_consent(
                principal.get_entity_id(),
                &client.client_id,
                &request.redirect_uri,
                &normalized_scope,
            )
            .await
            .map_err(ErrorResponse::from)?;

        Ok(IsAllowedForUserResponse {
            allowed: consent.is_some(),
        })
    }

    /// # Errors
    /// Returns an OAuth error response if Client authentication, grant validation, or issuance fails.
    pub async fn token(
        &self,
        request: TokenRequest,
        client_auth: Option<OAuth2ClientAuth>,
    ) -> ErrorResponseResult<TokenResponse> {
        if self.oauth_config.trusted_issuer_jwks.is_some() {
            return Err(
                ErrorResponse::new(ErrorCode::AccessDenied).with_description(
                    "this Node verifies tokens; request issuance at the canonical issuer",
                ),
            );
        }
        match request {
            TokenRequest::Password(request) => self.password(request, client_auth.as_ref()).await,
            TokenRequest::AuthorizationCode(request) => {
                self.authorization_code(request, client_auth.as_ref()).await
            }
            TokenRequest::ClientCredentials(request) => {
                self.client_credentials(request, client_auth.as_ref()).await
            }
            TokenRequest::RefreshToken(request) => {
                self.refresh_token(request, client_auth.as_ref()).await
            }
            TokenRequest::TokenExchange(request) => {
                self.token_exchange(request, client_auth.as_ref()).await
            }
        }
    }

    async fn password(
        &self,
        request: PasswordGrantRequest,
        client_auth: Option<&OAuth2ClientAuth>,
    ) -> ErrorResponseResult<TokenResponse> {
        let client = self
            .client_repo
            .find_client_by_client_id(&request.client_id)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::InvalidClient)
                    .with_description(format!("client {} not found", request.client_id))
            })?;
        Self::validate_grant_type(&client, GrantType::Password)?;
        Self::authenticate_client_for_token_endpoint(&client, client_auth)?;

        let user = self
            .user_repo
            .find_user_by_username_or_email(&request.username)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::InvalidGrant)
                    .with_description(format!("user {} not found", request.username))
            })?;

        let user_password = self
            .user_repo
            .find_user_password_hash(user.id)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::InvalidGrant)
                    .with_description("user password not found")
            })?;

        let verified_password =
            verify_password(&request.password, &user_password).map_err(|e| {
                ErrorResponse::new(ErrorCode::ServerError).with_description(e.to_string())
            })?;

        if !verified_password {
            return Err(ErrorResponse::new(ErrorCode::InvalidGrant)
                .with_description("invalid username or password"));
        }

        let key = self
            .key_service
            .key_repo()
            .find_by_entity_type_and_id(EntityType::User, user.id)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::InvalidGrant).with_description("user key not found")
            })?;

        let principal = self.find_principal(key.id).await?.ok_or_else(|| {
            ErrorResponse::new(ErrorCode::InvalidGrant)
                .with_description("principal not found for user")
        })?;

        let requested_scopes = request.scope.as_deref().map_or_default(parse_scopes);
        let scopes = intersect_scopes(&requested_scopes, &client.allowed_scopes);

        self.issue_tokens_for_client(
            &client,
            principal.as_ref(),
            &scopes,
            request.resource.as_deref(),
            None,
            None,
        )
        .await
    }

    async fn authorization_code(
        &self,
        request: AuthorizationCodeGrantRequest,
        client_auth: Option<&OAuth2ClientAuth>,
    ) -> ErrorResponseResult<TokenResponse> {
        let now = Utc::now();
        let authorization_code = self
            .authorization_code_repo
            .find_authorization_code_by_code(&request.code)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::InvalidGrant)
                    .with_description("authorization code not found")
            })?;

        if authorization_code.consumed_at.is_some() {
            return Err(ErrorResponse::new(ErrorCode::InvalidGrant)
                .with_description("authorization code already consumed"));
        }

        if authorization_code.expires_at <= now {
            return Err(ErrorResponse::new(ErrorCode::InvalidGrant)
                .with_description("authorization code expired"));
        }

        let principal = self
            .find_principal(authorization_code.key_id)
            .await?
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::InvalidGrant)
                    .with_description("principal not found for authorization code")
            })?;

        let client = self
            .client_repo
            .find_client_by_client_id(&authorization_code.client_id)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::InvalidClient).with_description("client not found")
            })?;
        Self::validate_grant_type(&client, GrantType::AuthorizationCode)?;
        Self::authenticate_client_for_token_endpoint(&client, client_auth)?;

        validate_authorization_code_grant(
            &request,
            &authorization_code.client_id,
            Some(&authorization_code.redirect_uri),
        )?;

        if let Some(code_challenge) = &authorization_code.code_challenge {
            verify_code_challenge(
                &request.code_verifier,
                code_challenge,
                authorization_code.code_challenge_method.ok_or_else(|| {
                    ErrorResponse::new(ErrorCode::InvalidGrant).with_description(
                        "code_challenge_method is required when code_challenge is present",
                    )
                })?,
            )?;
        }

        self.validate_issuance_inputs(&client, principal.as_ref(), &authorization_code.scopes)
            .await?;
        self.local_signing_jwk().await?;
        self.authorization_code_repo
            .consume_authorization_code(authorization_code.id, now)
            .await
            .map_err(ErrorResponse::from)?;

        self.issue_tokens_for_client(
            &client,
            principal.as_ref(),
            &authorization_code.scopes,
            authorization_code.resource.as_deref(),
            None,
            None,
        )
        .await
    }

    async fn client_credentials(
        &self,
        request: ClientCredentialsGrantRequest,
        client_auth: Option<&OAuth2ClientAuth>,
    ) -> ErrorResponseResult<TokenResponse> {
        #[cfg(feature = "std")]
        let observe = {
            let started = Instant::now();
            move |phase: &str| observe_client_credentials(started, phase)
        };
        #[cfg(not(feature = "std"))]
        let observe = |_phase: &str| {};
        observe("client_lookup");
        let auth = client_auth.as_ref().ok_or_else(|| {
            ErrorResponse::new(ErrorCode::InvalidClient)
                .with_description("client authentication is required")
        })?;

        let client = self
            .client_repo
            .find_client_by_client_id(&auth.client_id)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::InvalidClient).with_description("client not found")
            })?;
        Self::validate_grant_type(&client, GrantType::ClientCredentials)?;
        if client.client_type != ClientType::Confidential {
            return Err(ErrorResponse::new(ErrorCode::UnauthorizedClient)
                .with_description("client credentials requires a confidential client"));
        }
        if client
            .client_secret_expires_at
            .is_some_and(|expires_at| expires_at.timestamp() > 0 && expires_at <= Utc::now())
        {
            return Err(ErrorResponse::new(ErrorCode::InvalidClient)
                .with_description("client credentials have expired"));
        }
        observe("secret_verify");
        Self::authenticate_client_for_token_endpoint(&client, Some(auth))?;
        observe("grant_validation");
        if request.client_id != auth.client_id {
            return Err(ErrorResponse::new(ErrorCode::InvalidClient)
                .with_description("client_id does not match authenticated client"));
        }

        let audience = request.audience.as_deref().ok_or_else(|| {
            ErrorResponse::new(ErrorCode::InvalidRequest)
                .with_description("an audience is required for client credentials")
        })?;
        if !client
            .allowed_audiences
            .iter()
            .any(|allowed| allowed == audience)
        {
            return Err(ErrorResponse::new(ErrorCode::InvalidRequest)
                .with_description("client is not authorized for the requested audience"));
        }

        if request
            .resource
            .as_deref()
            .is_some_and(|resource| resource != audience)
        {
            return Err(ErrorResponse::new(ErrorCode::InvalidRequest)
                .with_description("resource must match the approved audience"));
        }

        let requested_scopes = request.scope.as_deref().map_or_default(parse_scopes);
        if requested_scopes.is_empty() {
            return Err(ErrorResponse::new(ErrorCode::InvalidScope)
                .with_description("at least one scope is required for client credentials"));
        }
        validate_scopes(&requested_scopes, &client.allowed_scopes)?;

        observe("active_root");
        let key = self
            .key_service
            .key_repo()
            .find_active_entity_root_key(EntityType::Client, client.id)
            .await?
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::InvalidClient)
                    .with_description("active client signing key not found")
            })?;
        observe("principal");
        let principal = self.find_principal(key.id).await?.ok_or_else(|| {
            ErrorResponse::new(ErrorCode::InvalidClient)
                .with_description("client principal is inactive")
        })?;
        observe("issue");
        let result = self
            .issue_client_credentials_access_token(
                &client,
                principal.as_ref(),
                &requested_scopes,
                audience,
                request.resource.as_deref(),
            )
            .await;
        observe("complete");
        result
    }

    async fn refresh_token(
        &self,
        request: RefreshTokenGrantRequest,
        client_auth: Option<&OAuth2ClientAuth>,
    ) -> ErrorResponseResult<TokenResponse> {
        let now = Utc::now();

        let (refresh_token, principal) = self
            .verify_token_principal(&request.refresh_token.0)
            .await
            .map_err(|_error| {
                ErrorResponse::new(ErrorCode::InvalidGrant)
                    .with_description("refresh token signature or Principal binding is invalid")
            })?;

        if refresh_token.r#type != TokenType::Bearer
            || refresh_token.r#use != TokenUse::Refresh
            || refresh_token.iss != self.oauth_config.issuer
            || refresh_token.nbf > now.timestamp()
            || refresh_token.iat > now.timestamp()
            || refresh_token.aud != refresh_token.client_id
            || refresh_token.principal_type != PrincipalType::User
        {
            return Err(ErrorResponse::new(ErrorCode::InvalidGrant)
                .with_description("refresh token claims are invalid"));
        }
        if refresh_token.exp <= now.timestamp() {
            return Err(ErrorResponse::new(ErrorCode::InvalidGrant)
                .with_description("refresh token is expired"));
        }
        let client = self
            .client_repo
            .find_client_by_client_id(&refresh_token.client_id)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::InvalidClient).with_description("client not found")
            })?;
        Self::validate_grant_type(&client, GrantType::RefreshToken)?;
        Self::authenticate_client_for_token_endpoint(&client, client_auth)?;

        let requested_scopes = request.scope.as_deref().map_or_default(parse_scopes);
        let scopes = if requested_scopes.is_empty() {
            refresh_token.scope.clone()
        } else {
            let scopes = intersect_scopes(&requested_scopes, &refresh_token.scope);
            if scopes.len() != requested_scopes.len() {
                return Err(ErrorResponse::new(ErrorCode::InvalidScope)
                    .with_description("requested scope must be a subset of refresh token scope"));
            }
            scopes
        };

        if principal.get_entity_id().to_string() != refresh_token.sub {
            return Err(ErrorResponse::new(ErrorCode::InvalidGrant)
                .with_description("refresh token subject does not match principal"));
        }
        validate_scopes(&scopes, &client.allowed_scopes)?;
        self.issue_tokens_with_refresh(
            &client,
            principal.as_ref(),
            &scopes,
            refresh_token.resource.as_deref(),
            refresh_token.authorization_details.as_deref(),
            None,
            Some(&request.refresh_token.0),
        )
        .await
    }

    async fn token_exchange(
        &self,
        request: TokenExchangeGrantRequest,
        client_auth: Option<&OAuth2ClientAuth>,
    ) -> ErrorResponseResult<TokenResponse> {
        if request.subject_token_type != SubjectTokenType::AccessToken {
            return Err(ErrorResponse::new(ErrorCode::InvalidRequest)
                .with_description("subject_token_type must be access_token"));
        }

        let access = self
            .verify_access_token_claims(&request.subject_token)
            .await?;
        let subject_token = access.standard_claims;
        let principal = self.principal_for_claims(&subject_token).await?;

        let client = self
            .client_repo
            .find_client_by_client_id(&subject_token.client_id)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::InvalidClient).with_description("client not found")
            })?;
        if Self::validate_bearer_registration(&subject_token, &client)? != access.application_id {
            return Err(ErrorResponse::new(ErrorCode::InvalidGrant));
        }
        Self::validate_grant_type(&client, GrantType::TokenExchange)?;
        Self::authenticate_client_for_token_endpoint(&client, client_auth)?;

        let requested_scopes = request.scope.as_deref().map_or_default(parse_scopes);
        let mut scopes = if requested_scopes.is_empty() {
            subject_token.scope.clone()
        } else {
            intersect_scopes(&requested_scopes, &subject_token.scope)
        };

        if let Some(details) = &request.authorization_details {
            validate_authorization_details(details)?;
        }

        let resource = request
            .resource
            .as_deref()
            .or(subject_token.resource.as_deref());

        if requested_scopes.is_empty()
            && resource == Some(MANAGEMENT_APPLICATION_URI)
            && client
                .allowed_scopes
                .iter()
                .any(|scope| scope == MANAGEMENT_POLICY_ADMINISTRATION_SCOPE)
            && !scopes
                .iter()
                .any(|scope| scope == MANAGEMENT_POLICY_ADMINISTRATION_SCOPE)
        {
            scopes.push(MANAGEMENT_POLICY_ADMINISTRATION_SCOPE.into());
        }

        self.issue_tokens_for_client(
            &client,
            principal.as_ref(),
            &scopes,
            resource,
            request.authorization_details.as_deref(),
            resource,
        )
        .await
    }

    fn validate_grant_type(client: &Client, grant_type: GrantType) -> ErrorResponseResult<()> {
        if client.allowed_grant_types.contains(&grant_type) {
            return Ok(());
        }

        Err(ErrorResponse::new(ErrorCode::UnauthorizedClient)
            .with_description("client is not authorized for this grant type"))
    }

    fn normalize_scopes(scopes: &[String]) -> String {
        let mut normalized = scopes.to_vec();
        normalized.sort();
        normalized.dedup();
        normalized.join(" ")
    }

    fn authenticate_client_for_token_endpoint(
        client: &Client,
        client_auth: Option<&OAuth2ClientAuth>,
    ) -> ErrorResponseResult<()> {
        match client.client_type {
            ClientType::Confidential => {
                let auth = client_auth.ok_or_else(|| {
                    ErrorResponse::new(ErrorCode::InvalidClient)
                        .with_description("client authentication is required")
                })?;

                if auth.client_id != client.client_id {
                    return Err(ErrorResponse::new(ErrorCode::InvalidClient)
                        .with_description("client_id does not match token subject"));
                }
                if auth.method != client.token_endpoint_auth_method
                    || !auth.method.uses_secret_verifier()
                {
                    return Err(ErrorResponse::new(ErrorCode::InvalidClient)
                        .with_description("unsupported client authentication method"));
                }

                let secret = auth.client_secret.as_deref().ok_or_else(|| {
                    ErrorResponse::new(ErrorCode::InvalidClient)
                        .with_description("client secret is required")
                })?;
                if !verify_password(secret, &client.client_secret_hash).map_err(|_error| {
                    ErrorResponse::new(ErrorCode::ServerError)
                        .with_description("stored client secret verifier is invalid")
                })? {
                    return Err(ErrorResponse::new(ErrorCode::InvalidClient)
                        .with_description("invalid client credentials"));
                }
            }
            ClientType::Public => {
                if let Some(auth) = client_auth
                    && auth.client_id != client.client_id
                {
                    return Err(ErrorResponse::new(ErrorCode::InvalidClient)
                        .with_description("client_id does not match token subject"));
                }
            }
        }
        Ok(())
    }

    /// # Errors
    /// Returns an OAuth error response if Client authentication or revocation persistence fails.
    pub async fn revoke(
        &self,
        request: RevocationRequest,
        client_auth: Option<OAuth2ClientAuth>,
    ) -> ErrorResponseResult<()> {
        if request.token.trim().is_empty() {
            return Err(
                ErrorResponse::new(ErrorCode::InvalidRequest).with_description("token is required")
            );
        }

        let Ok((header, _)) = decode_jwt::<StandardClaims>(&request.token) else {
            return Ok(());
        };
        let Ok(key_id) = Id::parse_str(&header.kid) else {
            return Ok(());
        };
        let Ok(key) = self.find_public_jwk(key_id).await else {
            return Ok(());
        };
        let Ok((_, claims)) = verify_jwt::<StandardClaims>(&key, &request.token) else {
            return Ok(());
        };
        if claims.r#type != TokenType::Bearer
            || claims.r#use != TokenUse::Refresh
            || claims.iss != self.oauth_config.issuer
            || claims.aud != claims.client_id
            || claims.principal_type != PrincipalType::User
        {
            return Ok(());
        }
        let Some(client) = self
            .client_repo
            .find_client_by_client_id(&claims.client_id)
            .await
            .map_err(ErrorResponse::from)?
        else {
            return Ok(());
        };
        Self::authenticate_client_for_token_endpoint(&client, client_auth.as_ref())?;
        self.refresh_token_repo
            .revoke_refresh_token(&request.token, client.id, Utc::now().timestamp())
            .await
            .map_err(ErrorResponse::from)
    }

    /// # Errors
    /// Returns an OAuth error response if trusted issuer keys cannot be loaded.
    pub async fn list_jwks(&self) -> ErrorResponseResult<Jwks> {
        if let Some(jwks) = &self.oauth_config.trusted_issuer_jwks {
            return Ok(jwks.clone());
        }
        let keys = self
            .key_service
            .key_repo()
            .issuer_keys(&self.oauth_config.issuer, Utc::now().timestamp())
            .await?;
        Ok(Jwks {
            keys: keys.into_iter().map(|(key, _)| key).collect(),
        })
    }

    /// # Errors
    /// Returns an OAuth error response if local signing or issuer key rotation fails.
    pub async fn rotate_issuer_key(&self) -> ErrorResponseResult<JwkPublic> {
        let previous = self.local_signing_jwk().await?;
        let mut binding = self
            .oauth_config
            .local_signer
            .clone()
            .ok_or_else(|| ErrorResponse::new(ErrorCode::AccessDenied))?;
        binding.key_id = Id::now_v7();
        let replacement = self.key_service.prepare_node_signer(&binding)?;
        let ttl = self
            .oauth_config
            .refresh_token_ttl_secs
            .max(self.oauth_config.token_ttl_secs);
        let expires = checked_expiration(Utc::now(), ttl)?.timestamp();
        self.key_service
            .key_repo()
            .rotate_issuer_key(
                &self.oauth_config.issuer,
                &replacement,
                &previous.kid,
                expires,
            )
            .await?;
        Ok(replacement)
    }

    /// # Errors
    /// Returns an OAuth error response if the Client or verifier update is invalid or fails.
    pub async fn rotate_client_secret(&self, client_id: &str) -> ErrorResponseResult<String> {
        let client = self
            .client_repo
            .find_client_registration(client_id)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| ErrorResponse::new(ErrorCode::NotFound))?;
        if client.client_type != ClientType::Confidential
            || !client.token_endpoint_auth_method.uses_secret_verifier()
        {
            return Err(ErrorResponse::new(ErrorCode::InvalidRequest)
                .with_description("secret rotation requires shared-secret Client authentication"));
        }
        let secret = generate_random_string::<64>();
        let hash = encrypt_password(&PasswordConfig::default(), &secret).map_err(|error| {
            ErrorResponse::new(ErrorCode::ServerError).with_description(error.to_string())
        })?;
        self.client_repo
            .update_client(client, Some(hash))
            .await
            .map_err(ErrorResponse::from)?;
        Ok(secret)
    }

    /// # Errors
    /// Returns an OAuth error response if the Client lookup or root key rotation fails.
    pub async fn rotate_client_key(&self, client_id: &str) -> ErrorResponseResult<Key> {
        let client = self
            .client_repo
            .find_client_by_client_id(client_id)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| ErrorResponse::new(ErrorCode::NotFound))?;
        let key = self
            .key_service
            .rotate_active_entity_root_key(
                EntityType::Client,
                client.id,
                "Client Principal root".into(),
                None,
            )
            .await
            .map_err(ErrorResponse::from)?;
        Ok(key)
    }

    /// # Errors
    /// Returns an OAuth error response if the Client lookup or key material deletion fails.
    pub async fn revoke_client_keys(&self, client_id: &str) -> ErrorResponseResult<()> {
        let client = self
            .client_repo
            .find_client_by_client_id(client_id)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| ErrorResponse::new(ErrorCode::NotFound))?;
        self.key_service
            .delete_entity_key_material(EntityType::Client, client.id)
            .await
            .map_err(ErrorResponse::from)
    }

    /// # Errors
    /// Returns an OAuth error response if the Client or active root key lookup fails.
    pub async fn list_client_keys(&self, client_id: &str) -> ErrorResponseResult<Vec<Key>> {
        let client = self
            .client_repo
            .find_client_by_client_id(client_id)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::InvalidClient).with_description("client not found")
            })?;

        let key = self
            .key_service
            .key_repo()
            .find_active_entity_root_key(EntityType::Client, client.id)
            .await
            .map_err(ErrorResponse::from)?;

        Ok(key.map_or_else(Vec::new, |key| vec![key]))
    }

    /// # Errors
    /// Returns an OAuth error response if the key is absent, ambiguous, or cannot be loaded.
    pub async fn find_public_jwk(&self, key_id: Id) -> ErrorResponseResult<JwkPublic> {
        let jwks = self.list_jwks().await?;
        let key_id = key_id.to_string();
        let mut matching = jwks.keys.into_iter().filter(|key| key.kid == key_id);
        let key = matching
            .next()
            .ok_or_else(|| ErrorResponse::new(ErrorCode::InvalidGrant))?;
        if matching.next().is_some() {
            return Err(ErrorResponse::new(ErrorCode::InvalidGrant));
        }
        Ok(key)
    }

    async fn validate_issuance_inputs(
        &self,
        client: &Client,
        principal: &dyn Principal,
        scopes: &[String],
    ) -> ErrorResponseResult<()> {
        let live_client = self
            .client_repo
            .find_client_by_client_id(&client.client_id)
            .await?
            .ok_or_else(|| ErrorResponse::new(ErrorCode::InvalidClient))?;
        if live_client != *client {
            return Err(ErrorResponse::new(ErrorCode::InvalidClient));
        }
        validate_scopes(scopes, &live_client.allowed_scopes)?;
        let live = self
            .find_principal(principal.get_key().id)
            .await?
            .ok_or_else(|| ErrorResponse::new(ErrorCode::InvalidGrant))?;
        if live.get_entity_id() != principal.get_entity_id()
            || live.get_entity_type() != principal.get_entity_type()
        {
            return Err(ErrorResponse::new(ErrorCode::InvalidGrant));
        }
        Ok(())
    }

    async fn local_signing_jwk(&self) -> ErrorResponseResult<JwkPrivate> {
        let binding = self.oauth_config.local_signer.as_ref().ok_or_else(|| {
            ErrorResponse::new(ErrorCode::AccessDenied)
                .with_description("local node signer is not configured")
        })?;
        self.key_service
            .load_node_signer(binding, &self.oauth_config.issuer, Utc::now().timestamp())
            .await
            .map_err(ErrorResponse::from)
    }

    async fn principal_for_claims(
        &self,
        claims: &StandardClaims,
    ) -> ErrorResponseResult<Box<dyn Principal>> {
        let id = Id::parse_str(&claims.sub)
            .map_err(|_error| ErrorResponse::new(ErrorCode::InvalidGrant))?;
        let entity_type = match claims.principal_type {
            PrincipalType::User => EntityType::User,
            PrincipalType::Client => EntityType::Client,
        };
        let key = self
            .key_service
            .key_repo()
            .find_active_entity_root_key(entity_type, id)
            .await?
            .ok_or_else(|| ErrorResponse::new(ErrorCode::InvalidGrant))?;
        let principal: Box<dyn Principal> = if entity_type == EntityType::Client {
            let client = self
                .client_repo
                .find_client_registration(&claims.client_id)
                .await?
                .ok_or_else(|| ErrorResponse::new(ErrorCode::InvalidClient))?;
            if client.id != id {
                return Err(ErrorResponse::new(ErrorCode::InvalidClient));
            }
            Box::new(ClientPrincipal { client, key })
        } else {
            self.principal_for_active_key(key)
                .await?
                .ok_or_else(|| ErrorResponse::new(ErrorCode::InvalidGrant))?
        };
        if principal.get_entity_id() != id || principal.get_entity_type() != entity_type {
            return Err(ErrorResponse::new(ErrorCode::InvalidGrant));
        }
        Ok(principal)
    }

    /// # Errors
    /// Returns an OAuth error response if token verification or Principal binding fails.
    pub async fn verify_token_principal(
        &self,
        token: &str,
    ) -> ErrorResponseResult<(StandardClaims, Box<dyn Principal>)> {
        let (header, _) = decode_jwt::<StandardClaims>(token)?;
        let key_id = Id::parse_str(&header.kid)
            .map_err(|_error| ErrorResponse::new(ErrorCode::InvalidGrant))?;
        let public = self.find_public_jwk(key_id).await?;
        let (_, bound) = verify_jwt::<super::token::PrincipalBoundClaims>(&public, token)?;
        let claims = bound.standard_claims;
        let now = Utc::now().timestamp();
        if claims.iss != self.oauth_config.issuer
            || claims.exp <= now
            || claims.nbf > now
            || claims.iat > now
        {
            return Err(ErrorResponse::new(ErrorCode::InvalidGrant));
        }
        let principal = self.principal_for_claims(&claims).await?;
        if bound.principal_key_id != principal.get_key().id {
            return Err(ErrorResponse::new(ErrorCode::InvalidGrant)
                .with_description("token Principal key is not the locally active root"));
        }
        Ok((claims, principal))
    }

    /// Verify the token before routing it to the operation that enforces its audience.
    ///
    /// # Errors
    /// Returns an OAuth error response if access token verification fails.
    pub async fn verify_access_token_claims(
        &self,
        token: &str,
    ) -> ErrorResponseResult<super::token::AccessClaims> {
        let (header, _) = decode_jwt::<IgnoredAny>(token)?;
        let key_id = Id::parse_str(&header.kid)
            .map_err(|_error| ErrorResponse::new(ErrorCode::InvalidGrant))?;
        let public = self.find_public_jwk(key_id).await?;
        super::token::verify_access_claims(
            &public,
            &self.oauth_config.issuer,
            token,
            Utc::now().timestamp(),
        )
    }

    /// # Errors
    /// Returns an OAuth error response if access token or audience validation fails.
    pub async fn validate_access_token(
        &self,
        token: &str,
        audience: &str,
    ) -> ErrorResponseResult<StandardClaims> {
        let access = self.verify_access_token_claims(token).await?;
        if access.standard_claims.aud != audience {
            return Err(ErrorResponse::new(ErrorCode::InvalidGrant));
        }
        Ok(access.standard_claims)
    }

    /// # Errors
    /// Returns an OAuth error response if no active User Principal can be resolved.
    pub async fn active_user_principal(
        &self,
        subject: Id,
    ) -> ErrorResponseResult<Box<dyn Principal>> {
        let key = self
            .key_service
            .key_repo()
            .find_active_entity_root_key(EntityType::User, subject)
            .await?
            .ok_or_else(|| ErrorResponse::new(ErrorCode::AccessDenied))?;
        self.principal_for_active_key(key)
            .await?
            .ok_or_else(|| ErrorResponse::new(ErrorCode::AccessDenied))
    }

    pub fn metadata(&self, endpoint_base: &str) -> AuthorizationServerMetadata {
        if self.oauth_config.trusted_issuer_jwks.is_some() {
            return self.oauth_config.to_metadata(&self.oauth_config.issuer);
        }
        self.oauth_config.to_metadata(endpoint_base)
    }

    /// # Errors
    /// Returns an OAuth error response if the device authorization request is invalid.
    pub fn device_authorization(
        &self,
        request: DeviceAuthorizationRequest,
    ) -> ErrorResponseResult<DeviceAuthorization> {
        if request
            .client_id
            .is_some_and(|client_id| client_id.is_empty())
        {
            return Err(ErrorResponse::new(ErrorCode::InvalidRequest)
                .with_description("client_id cannot be empty"));
        }

        let issuer = self.oauth_config.issuer.trim_end_matches('/');
        let user_code = generate_random_string::<32>();

        Ok(DeviceAuthorization {
            device_code: Some(generate_random_string::<32>()),
            expires_in: Some(self.oauth_config.device_code_ttl_secs),
            interval: Some(self.oauth_config.device_poll_interval_secs),
            user_code: Some(user_code.clone()),
            verification_uri: Some(format!("{issuer}/oauth2/device/verify")),
            verification_uri_complete: Some(format!(
                "{issuer}/oauth2/device/verify?user_code={user_code}"
            )),
        })
    }

    async fn issue_tokens_for_client(
        &self,
        client: &Client,
        principal: &dyn Principal,
        scopes: &[String],
        resource: Option<&str>,
        authorization_details: Option<&[AuthorizationDetail]>,
        audience: Option<&str>,
    ) -> ErrorResponseResult<TokenResponse> {
        self.issue_tokens_with_refresh(
            client,
            principal,
            scopes,
            resource,
            authorization_details,
            audience,
            None,
        )
        .await
    }

    async fn issue_tokens_with_refresh(
        &self,
        client: &Client,
        principal: &dyn Principal,
        scopes: &[String],
        resource: Option<&str>,
        authorization_details: Option<&[AuthorizationDetail]>,
        audience: Option<&str>,
        previous: Option<&str>,
    ) -> ErrorResponseResult<TokenResponse> {
        if principal.get_entity_type() != EntityType::User {
            return Err(ErrorResponse::new(ErrorCode::AccessDenied)
                .with_description("this grant cannot issue user tokens to a client principal"));
        }

        let now = Utc::now();
        self.validate_issuance_inputs(client, principal, scopes)
            .await?;
        let signing_jwk = self.local_signing_jwk().await?;
        let access_expires_at = checked_expiration(now, self.oauth_config.token_ttl_secs)?;
        let refresh_expires_at = checked_expiration(now, self.oauth_config.refresh_token_ttl_secs)?;
        let scope = if scopes.is_empty() {
            None
        } else {
            Some(scopes.join(" "))
        };

        let access_claims = StandardClaims {
            r#type: TokenType::Bearer,
            r#use: TokenUse::Access,
            exp: access_expires_at.timestamp(),
            iat: now.timestamp(),
            nbf: now.timestamp(),
            iss: self.oauth_config.issuer.clone(),
            aud: audience.unwrap_or(&client.client_id).to_string(),
            client_id: client.client_id.clone(),
            sub: principal.get_entity_id().to_string(),
            principal_type: match principal.get_entity_type() {
                EntityType::User => PrincipalType::User,
                EntityType::Client => PrincipalType::Client,
            },
            scope: scopes.to_vec(),
            resource: resource.map(str::to_string),
            authorization_details: authorization_details.map(<[AuthorizationDetail]>::to_vec),
        };

        let access_token_value = encode_jwt(
            &signing_jwk,
            &super::token::AccessClaims {
                standard_claims: access_claims.clone(),
                application_id: client.application_id,
                introspection_audiences: Vec::new(),
            },
        )?;

        let user_info = principal_user_info(principal);

        let id_token = IdTokenClaims {
            standard_claims: StandardClaims {
                r#use: TokenUse::Id,
                ..access_claims.clone()
            },
            principal_key_id: principal.get_key().id,
            user_info,
        };

        let id_token_value = encode_jwt(&signing_jwk, &id_token)?;

        let refresh_claims = StandardClaims {
            aud: client.client_id.clone(),
            r#use: TokenUse::Refresh,
            exp: refresh_expires_at.timestamp(),
            ..access_claims
        };

        let refresh_token_value = encode_jwt(
            &signing_jwk,
            &super::token::RefreshClaims {
                standard_claims: refresh_claims.clone(),
                principal_key_id: principal.get_key().id,
                jti: generate_random_string::<32>(),
            },
        )?;
        self.refresh_token_repo
            .issue_refresh_token(
                OAuth2RefreshToken {
                    token: refresh_token_value.clone(),
                    client_id: client.id,
                    user_id: principal.get_entity_id(),
                    scopes: refresh_claims.scope,
                    resource: refresh_claims.resource,
                    authorization_details: refresh_claims.authorization_details,
                    expires_at: refresh_claims.exp,
                    created_at: now.timestamp(),
                },
                previous,
            )
            .await
            .map_err(|error| {
                if previous.is_some() {
                    ErrorResponse::new(ErrorCode::InvalidGrant).with_description(error.to_string())
                } else {
                    ErrorResponse::from(error)
                }
            })?;

        Ok(TokenResponse {
            id_token: Some(IdToken(id_token_value)),
            access_token: AccessToken(access_token_value),
            token_type: TokenType::Bearer,
            expires_in: Some(self.oauth_config.token_ttl_secs),
            refresh_token_expires_in: Some(self.oauth_config.refresh_token_ttl_secs),
            refresh_token: Some(RefreshToken(refresh_token_value)),
            scope,
            issuer: Some(self.oauth_config.issuer.clone()),
        })
    }

    async fn issue_client_credentials_access_token(
        &self,
        client: &Client,
        principal: &dyn Principal,
        scopes: &[String],
        audience: &str,
        resource: Option<&str>,
    ) -> ErrorResponseResult<TokenResponse> {
        if principal.get_entity_type() != EntityType::Client
            || principal.get_entity_id() != client.id
        {
            return Err(ErrorResponse::new(ErrorCode::InvalidClient)
                .with_description("client signing principal does not match the client"));
        }

        let now = Utc::now();
        self.validate_issuance_inputs(client, principal, scopes)
            .await?;
        let signing_jwk = self.local_signing_jwk().await?;
        let claims = StandardClaims {
            r#type: TokenType::Bearer,
            r#use: TokenUse::Access,
            exp: checked_expiration(now, self.oauth_config.token_ttl_secs)?.timestamp(),
            iat: now.timestamp(),
            nbf: now.timestamp(),
            iss: self.oauth_config.issuer.clone(),
            aud: audience.to_string(),
            client_id: client.client_id.clone(),
            sub: client.id.to_string(),
            principal_type: PrincipalType::Client,
            scope: scopes.to_vec(),
            resource: resource.map(str::to_string),
            authorization_details: None,
        };
        let access_token = AccessToken(encode_jwt(
            &signing_jwk,
            &super::token::AccessClaims {
                standard_claims: claims,
                application_id: client.application_id,
                introspection_audiences: client.allowed_audiences.clone(),
            },
        )?);

        Ok(TokenResponse {
            id_token: None,
            access_token,
            token_type: TokenType::Bearer,
            expires_in: Some(self.oauth_config.token_ttl_secs),
            refresh_token: None,
            refresh_token_expires_in: None,
            scope: Some(scopes.join(" ")),
            issuer: Some(self.oauth_config.issuer.clone()),
        })
    }

    /// # Errors
    /// Returns an OAuth error response if User, root key, or profile persistence fails.
    pub async fn create_user(&self, request: CreateUserRequest) -> ErrorResponseResult<UserInfo> {
        let user = self
            .user_repo
            .create_user_with_password(&request.name, &request.password)
            .await
            .map_err(ErrorResponse::from)?;

        if !request.email.is_empty() {
            self.user_repo
                .upsert_primary_user_email(
                    user.id,
                    &request.email,
                    request.email_verified.unwrap_or(false),
                )
                .await
                .map_err(ErrorResponse::from)?;
        }

        self.key_service
            .create_initial_root_with_capture(
                EntityType::User,
                user.id,
                "Local User".into(),
                |_, _| Ok(()),
            )
            .await
            .map_err(ErrorResponse::from)?;

        self.hydrate_user_info(user).await
    }

    /// # Errors
    /// Returns an OAuth error response if the User is absent or profile lookup fails.
    pub async fn find_user_info(&self, user_id: Id) -> ErrorResponseResult<UserInfo> {
        let user = self
            .user_repo
            .find_user_by_id(user_id)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::NotFound)
                    .with_description("User not found".to_string())
            })?;

        self.hydrate_user_info(user).await
    }

    /// # Errors
    /// Returns an OAuth error response if Users or their profile data cannot be listed.
    pub async fn list_user_info(
        &self,
        offset: u32,
        limit: u32,
    ) -> ErrorResponseResult<Vec<UserInfo>> {
        let users = self
            .user_repo
            .list_users(offset, limit)
            .await
            .map_err(ErrorResponse::from)?;
        let mut user_info_list = Vec::with_capacity(users.len());

        for user in users {
            user_info_list.push(self.hydrate_user_info(user).await?);
        }

        Ok(user_info_list)
    }

    /// # Errors
    /// Returns an OAuth error response if profile validation, update, or hydration fails.
    pub async fn update_user_info(
        &self,
        user_id: Id,
        request: UpdateUserInfoRequest,
    ) -> ErrorResponseResult<UserInfo> {
        let existing = self
            .user_repo
            .find_user_by_id(user_id)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::NotFound)
                    .with_description("User not found".to_string())
            })?;

        let sex = match request.sex.as_deref() {
            Some("male") => Some(Sex::Male),
            Some("female") => Some(Sex::Female),
            Some(value) => {
                return Err(ErrorResponse::new(ErrorCode::InvalidRequest)
                    .with_description(format!("unsupported sex value: {value}")));
            }
            None => existing.sex,
        };

        let birthdate = match request.birthdate.as_deref() {
            Some(value) => Some(
                DateTime::parse_from_rfc3339(value)
                    .map_err(|_error| {
                        ErrorResponse::new(ErrorCode::InvalidRequest)
                            .with_description("birthdate must be RFC3339".to_string())
                    })?
                    .with_timezone(&Utc),
            ),
            None => existing.birthdate,
        };

        let user = User {
            id: existing.id,
            name: request.name.unwrap_or(existing.name),
            given_name: request.given_name.or(existing.given_name),
            family_name: request.family_name.or(existing.family_name),
            middle_name: request.middle_name.or(existing.middle_name),
            nickname: request.nickname.or(existing.nickname),
            profile: request.profile.or(existing.profile),
            picture: request.picture.or(existing.picture),
            website: request.website.or(existing.website),
            sex,
            birthdate,
            zoneinfo: request.zoneinfo.or(existing.zoneinfo),
            locale: request.locale.or(existing.locale),
            mfa_totp_secret: existing.mfa_totp_secret,
            mfa_webauthn_credentials: existing.mfa_webauthn_credentials,
            blocked: existing.blocked,
            blocked_at: existing.blocked_at,
            blocked_by: existing.blocked_by,
            created_at: existing.created_at,
            updated_at: Utc::now(),
        };

        let user_id = user.id;
        let user = self
            .user_repo
            .update_user(user)
            .await
            .map_err(ErrorResponse::from)?;

        if let Some(email) = request.email {
            self.user_repo
                .upsert_primary_user_email(user_id, &email, request.email_verified.unwrap_or(false))
                .await
                .map_err(ErrorResponse::from)?;
        }

        if let Some(phone_number) = request.phone_number {
            self.user_repo
                .upsert_primary_user_phone_number(
                    user_id,
                    &phone_number,
                    request.phone_number_verified.unwrap_or(false),
                )
                .await
                .map_err(ErrorResponse::from)?;
        }

        self.hydrate_user_info(user).await
    }

    /// # Errors
    /// Returns an OAuth error response if the User is absent or verifier replacement fails.
    pub async fn reset_user_password(
        &self,
        user_id: Id,
        password: &str,
    ) -> ErrorResponseResult<()> {
        if self
            .user_repo
            .find_user_by_id(user_id)
            .await
            .map_err(ErrorResponse::from)?
            .is_none()
        {
            return Err(ErrorResponse::new(ErrorCode::NotFound)
                .with_description("User not found".to_string()));
        }

        self.user_repo
            .replace_user_password(user_id, password)
            .await
            .map_err(ErrorResponse::from)
    }

    /// # Errors
    /// Returns an OAuth error response if the User is absent or deletion fails.
    pub async fn delete_user(&self, user_id: Id) -> ErrorResponseResult<()> {
        if self
            .user_repo
            .find_user_by_id(user_id)
            .await
            .map_err(ErrorResponse::from)?
            .is_none()
        {
            return Err(ErrorResponse::new(ErrorCode::NotFound)
                .with_description("User not found".to_string()));
        }

        self.user_repo
            .delete_user_by_id(user_id)
            .await
            .map_err(ErrorResponse::from)
    }

    /// # Errors
    /// Returns an OAuth error response if the User is absent or MFA persistence fails.
    pub async fn set_user_mfa_totp_secret(
        &self,
        user_id: Id,
        enable: bool,
    ) -> ErrorResponseResult<()> {
        let _user = self
            .user_repo
            .find_user_by_id(user_id)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::NotFound)
                    .with_description("User not found".to_string())
            })?;
        let secret = enable.then(generate_random_string::<32>);
        self.user_repo
            .set_user_mfa_totp_secret(user_id, secret)
            .await
            .map_err(ErrorResponse::from)
    }

    /// # Errors
    /// Returns an OAuth error response if the User is absent or MFA persistence fails.
    pub async fn set_user_mfa_webauthn_credentials(
        &self,
        user_id: Id,
        credentials: Option<String>,
    ) -> ErrorResponseResult<()> {
        if self
            .user_repo
            .find_user_by_id(user_id)
            .await
            .map_err(ErrorResponse::from)?
            .is_none()
        {
            return Err(ErrorResponse::new(ErrorCode::NotFound)
                .with_description("User not found".to_string()));
        }
        self.user_repo
            .set_user_mfa_webauthn_credentials(user_id, credentials)
            .await
            .map_err(ErrorResponse::from)
    }

    /// # Errors
    /// Returns an OAuth error response if the User is absent or blocked-state persistence fails.
    pub async fn set_user_blocked(
        &self,
        user_id: Id,
        blocked: bool,
        blocked_by: Option<Id>,
    ) -> ErrorResponseResult<()> {
        if self
            .user_repo
            .find_user_by_id(user_id)
            .await
            .map_err(ErrorResponse::from)?
            .is_none()
        {
            return Err(ErrorResponse::new(ErrorCode::NotFound)
                .with_description("User not found".to_string()));
        }
        self.user_repo
            .set_user_blocked(user_id, blocked, blocked_by)
            .await
            .map_err(ErrorResponse::from)
    }

    /// # Errors
    /// Returns an OAuth error response if the User is absent or consent lookup fails.
    pub async fn list_user_consents(
        &self,
        user_id: Id,
        offset: u32,
        limit: u32,
    ) -> ErrorResponseResult<Vec<OAuth2UserConsent>> {
        if self
            .user_repo
            .find_user_by_id(user_id)
            .await
            .map_err(ErrorResponse::from)?
            .is_none()
        {
            return Err(ErrorResponse::new(ErrorCode::NotFound)
                .with_description("User not found".to_string()));
        }

        self.oauth2_user_consent_repo
            .list_user_consents(user_id, offset, limit)
            .await
            .map_err(ErrorResponse::from)
    }

    /// # Errors
    /// Returns an OAuth error response if consent ownership validation or deletion fails.
    pub async fn revoke_user_consent(
        &self,
        user_id: Id,
        consent_id: Id,
    ) -> ErrorResponseResult<()> {
        if self
            .user_repo
            .find_user_by_id(user_id)
            .await
            .map_err(ErrorResponse::from)?
            .is_none()
        {
            return Err(ErrorResponse::new(ErrorCode::NotFound)
                .with_description("User not found".to_string()));
        }

        let consent = self
            .oauth2_user_consent_repo
            .find_user_consent_by_id(consent_id)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::NotFound)
                    .with_description("User consent not found".to_string())
            })?;

        if consent.user_id != user_id {
            return Err(ErrorResponse::new(ErrorCode::NotFound)
                .with_description("User consent not found".to_string()));
        }

        self.oauth2_user_consent_repo
            .delete_user_consent_by_id(consent_id)
            .await
            .map_err(ErrorResponse::from)
    }

    async fn hydrate_user_info(&self, user: User) -> ErrorResponseResult<UserInfo> {
        let user_id = user.id;
        let mut user_info: UserInfo = From::from(user);

        let emails = self
            .user_repo
            .find_user_emails_by_user_id(user_id)
            .await
            .map_err(ErrorResponse::from)?;
        if let Some(primary_email) = emails.into_iter().find(|e| e.primary) {
            user_info.email = Some(primary_email.email);
            user_info.email_verified = Some(primary_email.verified);
        }

        let phone_numbers = self
            .user_repo
            .find_user_phone_numbers_by_user_id(user_id)
            .await
            .map_err(ErrorResponse::from)?;
        if let Some(primary_phone_number) = phone_numbers.into_iter().find(|e| e.primary) {
            user_info.phone_number = Some(primary_phone_number.phone_number);
            user_info.phone_number_verified = Some(primary_phone_number.verified);
        }

        Ok(user_info)
    }

    /// # Errors
    /// Returns an OAuth error response if Principal or active root key lookup fails.
    pub async fn find_principal(
        &self,
        key_id: Id,
    ) -> ErrorResponseResult<Option<Box<dyn Principal>>> {
        let Some(key) = self.key_service.key_repo().find_by_id(key_id).await? else {
            return Ok(None);
        };

        if self.ensure_key_is_active_entity_root(&key).await.is_err() {
            return Ok(None);
        }

        self.principal_for_active_key(key).await
    }

    async fn principal_for_active_key(
        &self,
        key: Key,
    ) -> ErrorResponseResult<Option<Box<dyn Principal>>> {
        let principal = match key.entity_type {
            EntityType::User => {
                let Some(user) = self.user_repo.find_user_by_id(key.entity_id).await? else {
                    return Ok(None);
                };

                if user.blocked {
                    return Ok(None);
                }
                Box::new(UserPrincipal { user, key }) as Box<dyn Principal>
            }
            EntityType::Client => {
                let Some(client) = self.client_repo.find_client_by_id(key.entity_id).await? else {
                    return Ok(None);
                };

                Box::new(ClientPrincipal { client, key }) as Box<dyn Principal>
            }
        };

        Ok(Some(principal))
    }

    async fn ensure_key_is_active_entity_root(&self, key: &Key) -> ErrorResponseResult<()> {
        let active_root = self
            .key_service
            .key_repo()
            .find_active_entity_root_key(key.entity_type, key.entity_id)
            .await
            .map_err(ErrorResponse::from)?;

        let active_root = active_root.ok_or_else(|| {
            ErrorResponse::new(ErrorCode::InvalidGrant)
                .with_description("active entity root key not found")
        })?;

        let binding = TokenPrincipalBinding {
            entity_type: key.entity_type,
            entity_id: key.entity_id,
            key_id: key.id,
        };
        if !binding.matches_active_root(&active_root, Utc::now().timestamp()) {
            return Err(ErrorResponse::new(ErrorCode::InvalidGrant)
                .with_description("key is not the active entity root"));
        }

        Ok(())
    }
}

fn checked_expiration(now: DateTime<Utc>, ttl_secs: u64) -> ErrorResponseResult<DateTime<Utc>> {
    let seconds = i64::try_from(ttl_secs).map_err(|_error| invalid_token_ttl())?;
    let duration = Duration::try_seconds(seconds).ok_or_else(invalid_token_ttl)?;
    now.checked_add_signed(duration)
        .ok_or_else(invalid_token_ttl)
}

fn invalid_token_ttl() -> ErrorResponse {
    ErrorResponse::new(ErrorCode::ServerError)
        .with_description("configured token TTL is outside the supported timestamp range")
}

fn principal_user_info(principal: &dyn Principal) -> Option<UserInfo> {
    if principal.get_entity_type() != EntityType::User {
        return None;
    }
    principal
        .get_entity_as_any()
        .downcast_ref::<User>()
        .cloned()
        .map(User::into)
}

#[cfg(feature = "std")]
fn observe_client_credentials(started: Instant, phase: &str) {
    if env::var_os("OF_SETUP_TIMEOUT_TIMING").is_some() {
        let line = format!(
            "seam=client_credentials phase={phase} pid={} thread={:?} wall_ms={} elapsed_ms={}\n",
            process::id(),
            thread::current().id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_millis(),
            started.elapsed().as_millis()
        );
        let _ = io::stderr().lock().write_all(line.as_bytes());
    }
}

#[cfg(test)]
mod ttl_tests {
    use chrono::{DateTime, Duration};
    use of_model::idp::contract::error_code::ErrorCode;

    use super::checked_expiration;

    #[test]
    fn maximum_ttl_returns_server_error_instead_of_panicking() {
        let now = DateTime::from_timestamp(1_700_000_000, 0).expect("valid test timestamp");

        let error = checked_expiration(now, u64::MAX).expect_err("maximum TTL must be rejected");

        assert_eq!(error.error, ErrorCode::ServerError);
        assert_eq!(
            error.error_description.as_deref(),
            Some("configured token TTL is outside the supported timestamp range")
        );
    }

    #[test]
    fn normal_ttl_preserves_the_exact_expiration() {
        let now =
            DateTime::from_timestamp(1_700_000_000, 123_456_789).expect("valid test timestamp");

        let expires = checked_expiration(now, 3600).expect("normal TTL must remain valid");

        assert_eq!(expires, now + Duration::seconds(3600));
    }
}
