use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use idp_service::oauth2::{decode_jwt, verify_jwt};

use idp_model::{
    contract::{IntrospectionRequest, IntrospectionResponse, Jwks},
    model::Id,
};
use iroh::EndpointId;
use model::contract::{
    AuthorizationDetail, StandardClaims, StorageAuthorizationAction, TokenResponse, TokenType,
    TokenUse,
};
use reqwest::{Client, Url, redirect::Policy};
use serde::Deserialize;
use storage_model::ResourceKind;

use crate::{DeviceEndpointIdentity, DeviceIdentity, PairingAcceptanceController};
#[cfg(feature = "replica")]
use crate::{DeviceRepo, DeviceState, replica::DbDeviceRepo};

#[derive(Clone)]
pub struct HostedControlPlane {
    idp_base_url: Url,
    storage_base_url: Url,
    expected_issuer: String,
    permission_evaluator_client_id: Option<String>,
    service_token_client: Option<Arc<ServiceTokenClient>>,
    client: Client,
    local_idp: Option<LocalIdp>,
    #[cfg(feature = "replica")]
    local_devices: Option<Arc<DbDeviceRepo>>,
    device_identity: Option<Arc<DeviceIdentity>>,
    pairing_acceptance: Option<Arc<dyn PairingAcceptanceController>>,
}

type LocalCall<I, O> =
    Arc<dyn Fn(I) -> Pin<Box<dyn Future<Output = Result<O, String>> + Send>> + Send + Sync>;

#[derive(Clone)]
struct LocalIdp {
    inspect: LocalCall<String, IntrospectionResponse>,
    active_user: LocalCall<Id, ()>,
}

impl HostedControlPlane {
    pub fn new(base_url: &str) -> Result<Self, String> {
        Self::new_with_services(base_url, base_url, base_url.trim_end_matches('/'))
    }

    pub fn new_with_issuer(base_url: &str, expected_issuer: &str) -> Result<Self, String> {
        Self::new_with_services(base_url, base_url, expected_issuer)
    }

    pub fn new_with_services(
        idp_base_url: &str,
        storage_base_url: &str,
        expected_issuer: &str,
    ) -> Result<Self, String> {
        let issuer = Url::parse(expected_issuer)
            .map_err(|_| "issuer must be an HTTP URL without a query or fragment".to_owned())?;
        if !matches!(issuer.scheme(), "http" | "https")
            || issuer.host().is_none()
            || issuer.query().is_some()
            || issuer.fragment().is_some()
            || !issuer.username().is_empty()
            || issuer.password().is_some()
        {
            return Err("issuer must be an HTTP URL without a query or fragment".to_owned());
        }
        let idp_base_url = normalize_service_url(idp_base_url)?;
        let storage_base_url = normalize_service_url(storage_base_url)?;
        Ok(Self {
            idp_base_url,
            storage_base_url,
            expected_issuer: expected_issuer.to_owned(),
            permission_evaluator_client_id: None,
            service_token_client: None,
            local_idp: None,
            #[cfg(feature = "replica")]
            local_devices: None,
            device_identity: None,
            pairing_acceptance: None,
            client: Client::builder()
                .connect_timeout(Duration::from_secs(3))
                .timeout(Duration::from_secs(10))
                .redirect(Policy::none())
                .build()
                .map_err(|error| error.to_string())?,
        })
    }

    /// Callbacks must validate against the current local IdP owner on every call.
    pub fn with_local_idp<F, T, U, V>(mut self, inspect: F, active_user: U) -> Self
    where
        F: Fn(String) -> T + Send + Sync + 'static,
        T: Future<Output = Result<IntrospectionResponse, String>> + Send + 'static,
        U: Fn(Id) -> V + Send + Sync + 'static,
        V: Future<Output = Result<(), String>> + Send + 'static,
    {
        self.local_idp = Some(LocalIdp {
            inspect: Arc::new(move |token| Box::pin(inspect(token))),
            active_user: Arc::new(move |id| Box::pin(active_user(id))),
        });
        self
    }

    #[cfg(feature = "replica")]
    pub fn with_local_devices(mut self, devices: Arc<DbDeviceRepo>) -> Self {
        self.local_devices = Some(devices);
        self
    }

    #[cfg(feature = "replica")]
    pub fn device_repo(&self) -> Result<&Arc<DbDeviceRepo>, String> {
        self.local_devices
            .as_ref()
            .ok_or_else(|| "Management Device owner is unavailable".into())
    }

    pub fn with_device_identity(mut self, identity: Arc<DeviceIdentity>) -> Self {
        self.device_identity = Some(identity);
        self
    }

    pub fn device_identity(&self) -> Result<&DeviceIdentity, String> {
        self.device_identity
            .as_deref()
            .ok_or_else(|| "local endpoint identity is unavailable".into())
    }

    pub fn with_pairing_acceptance(
        mut self,
        controller: Arc<dyn PairingAcceptanceController>,
    ) -> Self {
        self.pairing_acceptance = Some(controller);
        self
    }

    pub fn pairing_acceptance(&self) -> Result<&dyn PairingAcceptanceController, String> {
        self.pairing_acceptance
            .as_deref()
            .ok_or_else(|| "pairing controller is unavailable".into())
    }

    pub async fn active_user(&self, user: Id) -> Result<(), String> {
        if user.is_nil() {
            return Err("invalid User ID".into());
        }
        if let Some(local) = &self.local_idp {
            return (local.active_user)(user).await;
        }
        let token = self.idp_service_access_token().await?;
        let mut response = self
            .client
            .get(service_url(
                &self.idp_base_url,
                &format!("users/{user}/active"),
            )?)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|_| "IdP User lookup is unavailable".to_owned())?;
        if response.status() != reqwest::StatusCode::NO_CONTENT {
            return Err("IdP User is not currently active or lookup is unavailable".into());
        }
        let body = read_limited_body(&mut response, "IdP User response").await?;
        if !body.is_empty() {
            return Err("invalid IdP User response".into());
        }
        Ok(())
    }

    pub fn with_permission_evaluator(mut self, client_id: &str) -> Result<Self, String> {
        if client_id.trim().is_empty()
            || self
                .service_token_client
                .as_ref()
                .is_some_and(|client| client.client_id == client_id)
        {
            return Err("IdP permission evaluator must use a distinct client ID".to_owned());
        }
        self.permission_evaluator_client_id = Some(client_id.to_owned());
        Ok(self)
    }

    pub fn permission_evaluator_client_id(&self) -> Option<&str> {
        self.permission_evaluator_client_id.as_deref()
    }

    pub fn with_idp_service_client(
        self,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        audience: impl Into<String>,
    ) -> Result<Self, String> {
        self.with_scoped_service_client(
            client_id,
            client_secret,
            audience,
            "idp.token.validate idp.user.lookup",
        )
    }

    pub(crate) fn with_scoped_service_client(
        mut self,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        audience: impl Into<String>,
        scope: &'static str,
    ) -> Result<Self, String> {
        let client_id = client_id.into();
        let client_secret = client_secret.into();
        let audience = audience.into();
        if client_id.trim().is_empty()
            || client_secret.is_empty()
            || audience.trim().is_empty()
            || self.permission_evaluator_client_id.as_deref() == Some(client_id.as_str())
        {
            return Err("IdP service client ID, secret and audience are required".to_owned());
        }
        self.service_token_client = Some(Arc::new(ServiceTokenClient {
            client_id,
            client_secret,
            audience,
            scope,
            cached: Mutex::new(None),
        }));
        Ok(self)
    }

    pub(crate) async fn idp_service_access_token(&self) -> Result<String, String> {
        const RENEWAL_MARGIN: Duration = Duration::from_secs(15);

        let token_client = self
            .service_token_client
            .as_ref()
            .ok_or_else(|| "IdP service OAuth client is not configured".to_owned())?;
        let cached_token = {
            let cached = token_client
                .cached
                .lock()
                .map_err(|_| "IdP service token cache is unavailable".to_owned())?;
            cached
                .as_ref()
                .filter(|token| {
                    token
                        .valid_until
                        .duration_since(SystemTime::now())
                        .is_ok_and(|remaining| remaining > RENEWAL_MARGIN)
                })
                .map(|token| token.value.clone())
        };
        if let Some(token) = cached_token {
            return Ok(token);
        }

        let form = [
            ("grant_type".to_owned(), "client_credentials".to_owned()),
            ("client_id".to_owned(), token_client.client_id.clone()),
            (
                "client_secret".to_owned(),
                token_client.client_secret.clone(),
            ),
            ("scope".to_owned(), token_client.scope.to_owned()),
            ("audience".to_owned(), token_client.audience.clone()),
        ];
        let mut response = self
            .client
            .post(service_url(&self.idp_base_url, "oauth2/token")?)
            .form(&form)
            .send()
            .await
            .map_err(|_| "IdP token endpoint is unavailable".to_owned())?;
        if !response.status().is_success() {
            return Err("IdP rejected the Management service client".to_owned());
        }
        let body = read_limited_body(&mut response, "IdP token response").await?;
        let token: TokenResponse =
            serde_json::from_slice(&body).map_err(|_| "invalid IdP token response".to_owned())?;
        if token.token_type != TokenType::Bearer
            || token.issuer.as_deref() != Some(self.expected_issuer.as_str())
            || !token.scope.as_deref().is_some_and(|scope| {
                let granted = scope.split_ascii_whitespace().collect::<Vec<_>>();
                token_client
                    .scope
                    .split_ascii_whitespace()
                    .all(|required| granted.contains(&required))
            })
        {
            return Err("IdP returned an unauthorized service token".to_owned());
        }
        let expires_in = token
            .expires_in
            .ok_or_else(|| "IdP service token has no expiry".to_owned())?;
        let valid_until = SystemTime::now()
            .checked_add(Duration::from_secs(expires_in))
            .ok_or_else(|| "IdP service token expiry is invalid".to_owned())?;
        let value = token.access_token.0;
        *token_client
            .cached
            .lock()
            .map_err(|_| "IdP service token cache is unavailable".to_owned())? =
            Some(CachedServiceToken {
                value: value.clone(),
                valid_until,
            });
        Ok(value)
    }

    /// Validates an actor token through the local IdP owner or authenticated introspection.
    pub async fn validate_actor_token(
        &self,
        actor_token: &str,
    ) -> Result<(StandardClaims, Id), String> {
        let response = match &self.local_idp {
            Some(local) => (local.inspect)(actor_token.to_owned()).await?,
            None => self.introspect_actor_token(actor_token).await?,
        };
        self.validate_actor_response(response)
    }

    async fn introspect_actor_token(
        &self,
        actor_token: &str,
    ) -> Result<IntrospectionResponse, String> {
        let service_token = self.idp_service_access_token().await?;
        let request = IntrospectionRequest {
            token: actor_token.to_owned(),
            token_type_hint: Some("access_token".to_owned()),
        };
        let mut response = self
            .client
            .post(service_url(&self.idp_base_url, "oauth2/introspect")?)
            .bearer_auth(service_token)
            .json(&request)
            .send()
            .await
            .map_err(|_| "IdP token validation is unavailable".to_owned())?;
        if response.status().as_u16() == 401 {
            return Err("actor token was rejected by IdP".to_owned());
        }
        if !response.status().is_success() {
            return Err("IdP token validation is unavailable".to_owned());
        }
        const MAX_RESPONSE_SIZE: usize = 64 * 1024;
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "invalid IdP token validation response".to_owned())?
        {
            if body.len() + chunk.len() > MAX_RESPONSE_SIZE {
                return Err("IdP token validation response is too large".to_owned());
            }
            body.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&body)
            .map_err(|_| "invalid IdP token validation response".to_owned())
    }

    fn validate_actor_response(
        &self,
        response: IntrospectionResponse,
    ) -> Result<(StandardClaims, Id), String> {
        let application_id = response
            .application_id
            .parse()
            .map_err(|_| "invalid application ID in IdP response".to_owned())?;
        self.check_issuer(&response.claims)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "invalid local clock".to_owned())?
            .as_secs() as i64;
        if response.claims.r#type != TokenType::Bearer
            || response.claims.r#use != TokenUse::Access
            || response.claims.exp <= now
            || response.claims.nbf > now
            || response.claims.iat > now
            || response.claims.aud.is_empty()
            || response.claims.client_id.is_empty()
            || response.claims.sub.parse::<Id>().is_err()
            || application_id == Id::nil()
        {
            return Err("invalid claims in IdP token validation response".to_owned());
        }
        Ok((response.claims, application_id))
    }

    pub async fn get_storage_endpoint_identity(
        &self,
        endpoint_id: &str,
    ) -> Result<DeviceEndpointIdentity, String> {
        let endpoint = endpoint_id
            .parse::<EndpointId>()
            .map_err(|_| "invalid endpoint ID")?;
        if endpoint.to_string() != endpoint_id {
            return Err("invalid endpoint ID".into());
        }
        #[cfg(feature = "replica")]
        {
            let device = self
                .device_repo()?
                .find_approved_by_public_key(endpoint_id)
                .await
                .map_err(|error| error.to_string())?
                .ok_or("endpoint has no uniquely approved Management Device")?;
            let owner = device
                .owner_subject
                .parse::<Id>()
                .map_err(|_| "invalid Device owner")?;
            if device.id.is_nil() || owner.is_nil() || owner.to_string() != device.owner_subject {
                return Err("invalid Device owner binding".into());
            }
            self.active_user(owner).await?;
            Ok(DeviceEndpointIdentity {
                device_id: device.id,
                owner_subject: device.owner_subject,
                endpoint_id: endpoint_id.to_owned(),
            })
        }
        #[cfg(not(feature = "replica"))]
        Err("Management Device owner is unavailable".into())
    }

    #[cfg(feature = "replica")]
    pub async fn approved_device_endpoints(&self) -> Result<Vec<String>, String> {
        let mut endpoints = Vec::new();
        for device in self
            .device_repo()?
            .list()
            .await
            .map_err(|error| error.to_string())?
        {
            if device.state == DeviceState::Approved && device.revoked_at.is_none() {
                let identity = self
                    .get_storage_endpoint_identity(&device.public_key)
                    .await?;
                if identity.device_id != device.id {
                    return Err("ambiguous Management Device binding".into());
                }
                endpoints.push(identity.endpoint_id);
            }
        }
        Ok(endpoints)
    }

    pub async fn validate_storage_resource(
        &self,
        read_token: &str,
        expected_owner: &str,
        expected_storage_audience: &str,
        expected_application_id: Id,
        kind: ResourceKind,
        resource_id: &str,
    ) -> Result<Id, String> {
        let claims = self.verify_access_token(read_token).await?;
        validate_storage_read_claims(&claims, expected_owner, expected_storage_audience)?;
        self.lookup_storage_resource(read_token, expected_application_id, kind, resource_id)
            .await
    }

    async fn lookup_storage_resource(
        &self,
        read_token: &str,
        expected_application_id: Id,
        kind: ResourceKind,
        resource_id: &str,
    ) -> Result<Id, String> {
        let id: Id = resource_id
            .parse()
            .map_err(|_| "invalid storage resource ID".to_owned())?;
        if resource_id != id.to_string() {
            return Err("invalid storage resource ID".to_owned());
        }
        let collection = match kind {
            ResourceKind::Database => "databases",
            ResourceKind::FileSystem => "filesystems",
        };
        let resource: StorageResourceDetail = self
            .get_storage(&format!("{collection}/{id}"), read_token)
            .await?;
        if resource.id != resource_id {
            return Err("storage resource ID mismatch".to_owned());
        }
        if resource.application_id != expected_application_id {
            return Err("storage application ID mismatch".to_owned());
        }
        Ok(resource.application_id)
    }

    pub async fn get_storage_device_identity(
        &self,
        device_id: Id,
    ) -> Result<DeviceEndpointIdentity, String> {
        if device_id.is_nil() {
            return Err("invalid Device ID".into());
        }
        #[cfg(feature = "replica")]
        {
            let device = self
                .device_repo()?
                .list()
                .await
                .map_err(|error| error.to_string())?
                .into_iter()
                .find(|device| device.id == device_id)
                .ok_or("Management Device was not found")?;
            let identity = self
                .get_storage_endpoint_identity(&device.public_key)
                .await?;
            if identity.device_id != device_id {
                return Err("mismatched Management Device binding".into());
            }
            Ok(identity)
        }
        #[cfg(not(feature = "replica"))]
        Err("Management Device owner is unavailable".into())
    }

    pub async fn verify_access_token(&self, token: &str) -> Result<StandardClaims, String> {
        let claims = if self.local_idp.is_some() {
            self.validate_actor_token(token).await?.0
        } else {
            self.verify_remote_signature(token).await?
        };
        self.check_issuer(&claims)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "system clock is before Unix epoch".to_owned())?
            .as_secs() as i64;
        if claims.r#type != TokenType::Bearer
            || claims.r#use != TokenUse::Access
            || claims.exp <= now
            || claims.iat > now
            || claims.nbf > now
            || claims.sub.is_empty()
            || claims.aud.is_empty()
            || claims.principal_type != model::contract::PrincipalType::User
        {
            return Err("invalid storage access token".to_owned());
        }
        Ok(claims)
    }

    async fn verify_remote_signature(&self, token: &str) -> Result<StandardClaims, String> {
        let (header, _) =
            decode_jwt::<StandardClaims>(token).map_err(|_| "invalid token".to_owned())?;
        let jwks: Jwks = self.get_idp(".well-known/jwks.json", "").await?;
        let jwk = jwks
            .keys
            .iter()
            .find(|jwk| jwk.kid == header.kid)
            .ok_or_else(|| "token signing key is not trusted".to_owned())?;
        let (_, claims) =
            verify_jwt::<StandardClaims>(jwk, token).map_err(|_| "invalid token".to_owned())?;
        Ok(claims)
    }

    fn check_issuer(&self, claims: &StandardClaims) -> Result<(), String> {
        if claims.iss != self.expected_issuer {
            return Err("token issuer is not the configured issuer".to_owned());
        }
        Ok(())
    }

    async fn get_idp<T>(&self, path: &str, token: &str) -> Result<T, String>
    where
        T: serde::de::DeserializeOwned,
    {
        self.get_from(&self.idp_base_url, path, token).await
    }

    async fn get_storage<T>(&self, path: &str, token: &str) -> Result<T, String>
    where
        T: serde::de::DeserializeOwned,
    {
        self.get_from(&self.storage_base_url, path, token).await
    }

    async fn get_from<T>(&self, base_url: &Url, path: &str, token: &str) -> Result<T, String>
    where
        T: serde::de::DeserializeOwned,
    {
        let mut request = self.client.get(service_url(base_url, path)?);
        if !token.is_empty() {
            request = request.bearer_auth(token);
        }
        request
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?
            .json()
            .await
            .map_err(|error| error.to_string())
    }
}

pub(crate) fn normalize_service_url(value: &str) -> Result<Url, String> {
    let mut url = Url::parse(value).map_err(|_| "invalid service URI".to_owned())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host().is_none()
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || (url.scheme() == "http" && !is_loopback(&url))
    {
        return Err("service URI must use HTTPS except for loopback and must not contain credentials, a query, or a fragment".to_owned());
    }
    if !url.path().ends_with('/') {
        url.set_path(&format!("{}/", url.path()));
    }
    Ok(url)
}

fn is_loopback(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    host.eq_ignore_ascii_case("localhost")
        || host.to_ascii_lowercase().ends_with(".localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn service_url(base_url: &Url, path: &str) -> Result<Url, String> {
    base_url.join(path).map_err(|error| error.to_string())
}

struct ServiceTokenClient {
    client_id: String,
    client_secret: String,
    audience: String,
    scope: &'static str,
    cached: Mutex<Option<CachedServiceToken>>,
}

struct CachedServiceToken {
    value: String,
    valid_until: SystemTime,
}

pub(crate) async fn read_limited_body(
    response: &mut reqwest::Response,
    label: &str,
) -> Result<Vec<u8>, String> {
    const MAX_RESPONSE_SIZE: usize = 64 * 1024;
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| format!("invalid {label}"))?
    {
        if body.len() + chunk.len() > MAX_RESPONSE_SIZE {
            return Err(format!("{label} is too large"));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[derive(Deserialize)]
struct StorageResourceDetail {
    id: String,
    #[serde(rename = "applicationId")]
    application_id: Id,
}

fn validate_storage_read_claims(
    claims: &StandardClaims,
    expected_owner: &str,
    expected_storage_audience: &str,
) -> Result<(), String> {
    let [AuthorizationDetail::Storage(detail)] =
        claims.authorization_details.as_deref().unwrap_or(&[])
    else {
        return Err("storage read authorization required".to_owned());
    };
    if expected_owner.is_empty()
        || claims.sub != expected_owner
        || claims.client_id.is_empty()
        || expected_storage_audience.is_empty()
        || claims.aud != expected_storage_audience
        || claims.resource.as_deref() != Some(expected_storage_audience)
        || !detail.actions.contains(&StorageAuthorizationAction::Read)
    {
        return Err("storage read authorization required".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    use model::contract::{
        AuthorizationDetail, PrincipalType, StandardClaims, StorageAuthorizationAction,
        StorageAuthorizationDetail, TokenType, TokenUse,
    };
    use storage_model::ResourceKind;

    use super::{HostedControlPlane, normalize_service_url, validate_storage_read_claims};

    const ID: &str = "00000000-0000-0000-0000-000000000001";
    const APP_ID: &str = "00000000-0000-0000-0000-000000000003";

    fn claims() -> StandardClaims {
        StandardClaims {
            r#type: TokenType::Bearer,
            r#use: TokenUse::Access,
            exp: i64::MAX,
            iat: 0,
            nbf: 0,
            iss: "https://lidp.example".into(),
            aud: "storage".into(),
            client_id: "verified-client".into(),
            sub: "owner".into(),
            principal_type: PrincipalType::User,
            resource: Some("storage".into()),
            authorization_details: Some(vec![AuthorizationDetail::Storage(
                StorageAuthorizationDetail {
                    actions: vec![StorageAuthorizationAction::Read],
                },
            )]),
            scope: vec!["storage".into()],
        }
    }

    fn check(claims: &StandardClaims) -> Result<(), String> {
        validate_storage_read_claims(claims, "owner", "storage")
    }

    #[tokio::test]
    async fn local_idp_rechecks_owner_callbacks_without_service_credentials() {
        use idp_model::{contract::IntrospectionResponse, model::Id};
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        let active = Arc::new(AtomicBool::new(true));
        let current = active.clone();
        let control =
            HostedControlPlane::new_with_issuer("http://127.0.0.1:1", "https://lidp.example")
                .expect("unused remote")
                .with_local_idp(
                    |_| async {
                        let mut claims = claims();
                        claims.sub = ID.into();
                        Ok(IntrospectionResponse {
                            claims,
                            application_id: APP_ID.into(),
                        })
                    },
                    move |id| {
                        let active = current.clone();
                        async move {
                            assert_eq!(id, ID.parse::<Id>().expect("User ID"));
                            if active.load(Ordering::SeqCst) {
                                Ok(())
                            } else {
                                Err("inactive User".into())
                            }
                        }
                    },
                );
        assert!(control.validate_actor_token("actor").await.is_ok());
        assert!(control.verify_access_token("actor").await.is_ok());
        assert!(
            control
                .active_user(ID.parse().expect("User ID"))
                .await
                .is_ok()
        );
        active.store(false, Ordering::SeqCst);
        assert!(
            control
                .active_user(ID.parse().expect("User ID"))
                .await
                .is_err()
        );
        assert!(control.active_user(Id::nil()).await.is_err());
    }

    #[tokio::test]
    async fn local_idp_preserves_claim_and_identity_response_checks() {
        use idp_model::{contract::IntrospectionResponse, model::Id};

        let endpoint = iroh::SecretKey::from_bytes(&[8; 32]).public().to_string();
        let control = HostedControlPlane::new_with_services(
            "http://127.0.0.1:1",
            "http://127.0.0.1:1",
            "https://lidp.example",
        )
        .expect("unused HTTP endpoints")
        .with_local_idp(
            |token: String| async move {
                let mut claims = claims();
                claims.sub = ID.into();
                let mut application_id = APP_ID.to_owned();
                match token.as_str() {
                    "issuer" => claims.iss = "https://other.example".into(),
                    "expired" => claims.exp = 0,
                    "token-type" => claims.r#type = TokenType::DPoP,
                    "token-use" => claims.r#use = TokenUse::Refresh,
                    "future-nbf" => claims.nbf = i64::MAX,
                    "future-iat" => claims.iat = i64::MAX,
                    "audience" => claims.aud.clear(),
                    "client" => claims.client_id.clear(),
                    "subject" => claims.sub = "not-an-id".into(),
                    "application" => application_id = Id::nil().to_string(),
                    "invalid-application" => application_id = "not-an-id".into(),
                    "client-principal" => claims.principal_type = PrincipalType::Client,
                    _ => {}
                }
                Ok(IntrospectionResponse {
                    claims,
                    application_id,
                })
            },
            |_| async { Err("inactive User".into()) },
        );
        for token in [
            "issuer",
            "expired",
            "token-type",
            "token-use",
            "future-nbf",
            "future-iat",
            "audience",
            "client",
            "subject",
            "application",
            "invalid-application",
        ] {
            assert!(
                control.validate_actor_token(token).await.is_err(),
                "{token}"
            );
        }
        assert!(
            control
                .validate_actor_token("client-principal")
                .await
                .is_ok()
        );
        assert!(
            control
                .verify_access_token("client-principal")
                .await
                .is_err()
        );
        assert!(
            control
                .get_storage_endpoint_identity(&endpoint)
                .await
                .is_err()
        );
        for device in [
            Id::nil(),
            ID.parse().expect("Device ID"),
            APP_ID.parse().expect("Application ID"),
        ] {
            assert!(control.get_storage_device_identity(device).await.is_err());
        }
    }

    #[test]
    fn storage_read_requires_owner_audience_resource_and_read_detail() {
        let valid = claims();
        assert!(check(&valid).is_ok());
        assert!(validate_storage_read_claims(&valid, "other", "storage").is_err());
        assert!(validate_storage_read_claims(&valid, "owner", "other").is_err());
        assert!(validate_storage_read_claims(&valid, "", "storage").is_err());
        assert!(validate_storage_read_claims(&valid, "owner", "").is_err());
        let mut invalid = valid.clone();
        invalid.client_id.clear();
        assert!(check(&invalid).is_err());
        invalid = valid.clone();
        invalid.resource = None;
        assert!(check(&invalid).is_err());
        invalid = valid.clone();
        invalid.authorization_details = None;
        assert!(check(&invalid).is_err());
        invalid.authorization_details = Some(vec![AuthorizationDetail::Storage(
            StorageAuthorizationDetail {
                actions: vec![StorageAuthorizationAction::Write],
            },
        )]);
        assert!(check(&invalid).is_err());
        invalid.authorization_details = Some(vec![
            AuthorizationDetail::Storage(StorageAuthorizationDetail {
                actions: vec![StorageAuthorizationAction::Read],
            }),
            AuthorizationDetail::Storage(StorageAuthorizationDetail {
                actions: vec![StorageAuthorizationAction::Read],
            }),
        ]);
        assert!(check(&invalid).is_err());
    }

    #[tokio::test]
    async fn introspection_rejection_is_not_treated_as_upstream_unavailable() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test HTTP server");
        let address = listener.local_addr().expect("read test HTTP address");
        let server = thread::spawn(move || {
            let (mut token_stream, _) = listener.accept().expect("accept token request");
            let mut token_request = [0; 4096];
            token_stream
                .read(&mut token_request)
                .expect("read token request");
            let token_response = r#"{"access_token":"management-token","token_type":"Bearer","expires_in":300,"scope":"idp.token.validate idp.user.lookup","iss":"https://issuer.example"}"#;
            write!(token_stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{token_response}", token_response.len()).expect("write token response");

            let (mut introspection_stream, _) =
                listener.accept().expect("accept introspection request");
            let mut introspection_request = [0; 4096];
            introspection_stream
                .read(&mut introspection_request)
                .expect("read introspection request");
            let error_response = r#"{"error":"invalid_token"}"#;
            write!(introspection_stream, "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{error_response}", error_response.len()).expect("write rejection response");
        });

        let base_url = format!("http://{address}");
        let control_plane =
            HostedControlPlane::new_with_services(&base_url, &base_url, "https://issuer.example")
                .expect("create hosted control plane")
                .with_idp_service_client("management-client", "management-secret", "idp-audience")
                .expect("configure IdP service client");

        let error = control_plane
            .validate_actor_token("revoked-or-invalid-token")
            .await
            .expect_err("IdP rejection must fail validation");
        assert_eq!(error, "actor token was rejected by IdP");
        server.join().expect("join test HTTP server");
    }

    async fn lookup_with_response(
        kind: ResourceKind,
        status: &str,
        body: &str,
    ) -> (Result<idp_model::model::Id, String>, String) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test HTTP server");
        let address = listener.local_addr().expect("read test HTTP address");
        let status = status.to_owned();
        let body = body.to_owned();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept test request");
            let mut request = [0; 4096];
            let length = stream.read(&mut request).expect("read test request");
            let request = String::from_utf8_lossy(&request[..length]).into_owned();
            write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).expect("write test response");
            request
        });
        let control_plane = HostedControlPlane::new(&format!("http://{address}"))
            .expect("create test control plane");
        let result = control_plane
            .lookup_storage_resource(
                "secret",
                APP_ID.parse().expect("valid application ID"),
                kind,
                ID,
            )
            .await;
        (result, server.join().expect("join test HTTP server"))
    }

    #[tokio::test]
    async fn active_user_lookup_uses_only_the_authenticated_idp_user_contract() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("HTTP seam");
        let address = listener.local_addr().expect("address");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("token");
            let mut request = [0; 4096];
            let n = stream.read(&mut request).expect("token request");
            let token_request = String::from_utf8_lossy(&request[..n]).into_owned();
            let body = r#"{"access_token":"service-token","token_type":"Bearer","expires_in":300,"scope":"idp.token.validate idp.user.lookup","iss":"https://issuer.example"}"#;
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).expect("token response");
            let (mut stream, _) = listener.accept().expect("User lookup");
            let n = stream.read(&mut request).expect("User request");
            let lookup_request = String::from_utf8_lossy(&request[..n]).into_owned();
            write!(
                stream,
                "HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n"
            )
            .expect("active User response");
            (token_request, lookup_request)
        });
        let base = format!("http://{address}");
        let control = HostedControlPlane::new_with_issuer(&base, "https://issuer.example")
            .expect("control")
            .with_idp_service_client("management", "secret", "idp-services")
            .expect("service Client");
        control
            .active_user(ID.parse().expect("User ID"))
            .await
            .expect("current active User");
        let (token, lookup) = server.join().expect("HTTP seam");
        assert!(token.contains("scope=idp.token.validate+idp.user.lookup"));
        assert!(lookup.starts_with(&format!("GET /users/{ID}/active HTTP/1.1")));
        assert!(
            lookup
                .to_ascii_lowercase()
                .contains("authorization: bearer service-token")
        );
    }

    #[tokio::test]
    async fn actor_validation_uses_scoped_client_bearer_and_returns_verified_claims() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test HTTP server");
        let address = listener.local_addr().expect("read test HTTP address");
        let mut actor_claims = claims();
        actor_claims.iss = "https://issuer.example".into();
        actor_claims.sub = ID.into();
        let introspection_body =
            serde_json::json!({ "claims": actor_claims, "applicationId": APP_ID }).to_string();
        let server = thread::spawn(move || {
            let (mut token_stream, _) = listener.accept().expect("accept token request");
            let mut token_request = [0; 4096];
            let token_length = token_stream
                .read(&mut token_request)
                .expect("read token request");
            let token_request =
                String::from_utf8_lossy(&token_request[..token_length]).into_owned();
            let token_body = serde_json::json!({
                "access_token": "service-token",
                "token_type": "Bearer",
                "expires_in": 300,
                "scope": "idp.token.validate idp.user.lookup",
                "iss": "https://issuer.example"
            })
            .to_string();
            write!(token_stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{token_body}", token_body.len()).expect("write token response");

            let (mut introspection_stream, _) =
                listener.accept().expect("accept introspection request");
            let mut introspection_request = [0; 4096];
            let introspection_length = introspection_stream
                .read(&mut introspection_request)
                .expect("read introspection request");
            let introspection_request =
                String::from_utf8_lossy(&introspection_request[..introspection_length])
                    .into_owned();
            write!(introspection_stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{introspection_body}", introspection_body.len()).expect("write introspection response");
            (token_request, introspection_request)
        });
        let endpoint = format!("http://{address}");
        let control_plane =
            HostedControlPlane::new_with_services(&endpoint, &endpoint, "https://issuer.example")
                .expect("create test control plane")
                .with_idp_service_client("management-client", "management-secret", "idp-audience")
                .expect("configure service OAuth client");
        let (validated, application_id) = control_plane
            .validate_actor_token("actor-token")
            .await
            .expect("IdP validates actor token");
        let (token_request, introspection_request) = server.join().expect("join test HTTP server");
        assert_eq!(validated.client_id, "verified-client");
        assert_eq!(application_id.to_string(), APP_ID);
        assert!(token_request.starts_with("POST /oauth2/token HTTP/1.1"));
        assert!(token_request.contains("grant_type=client_credentials"));
        assert!(token_request.contains("scope=idp.token.validate+idp.user.lookup"));
        assert!(token_request.contains("audience=idp-audience"));
        assert!(introspection_request.starts_with("POST /oauth2/introspect HTTP/1.1"));
        assert!(
            introspection_request
                .to_ascii_lowercase()
                .contains("authorization: bearer service-token")
        );
        assert!(introspection_request.contains("actor-token"));
        assert!(
            !introspection_request
                .to_ascii_lowercase()
                .contains("x-internal-service")
        );
    }

    #[tokio::test]
    async fn lookup_uses_kind_specific_get_and_returns_application_id() {
        for (kind, path) in [
            (ResourceKind::Database, "databases"),
            (ResourceKind::FileSystem, "filesystems"),
        ] {
            let (result, request) = lookup_with_response(
                kind,
                "200 OK",
                &format!(r#"{{"id":"{ID}","applicationId":"{APP_ID}"}}"#),
            )
            .await;
            assert_eq!(result, Ok(APP_ID.parse().expect("valid application ID")));
            assert!(request.starts_with(&format!("GET /{path}/{ID} HTTP/1.1")));
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer secret")
            );
        }
    }

    #[tokio::test]
    async fn lookup_rejects_wrong_or_missing_id_and_application_id() {
        for body in [
            format!(
                r#"{{"id":"00000000-0000-0000-0000-000000000002","applicationId":"{APP_ID}"}}"#
            ),
            format!(r#"{{"applicationId":"{APP_ID}"}}"#),
            format!(r#"{{"id":"{ID}","applicationId":"00000000-0000-0000-0000-000000000004"}}"#),
            format!(r#"{{"id":"{ID}"}}"#),
            format!(r#"{{"id":"{ID}","applicationId":"not-a-uuid"}}"#),
        ] {
            let (result, _) = lookup_with_response(ResourceKind::Database, "200 OK", &body).await;
            assert!(result.is_err(), "unexpectedly accepted: {body}");
        }
        let (result, _) =
            lookup_with_response(ResourceKind::FileSystem, "404 Not Found", "{}").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn lookup_rejects_invalid_id_and_unavailable_api() {
        let control_plane =
            HostedControlPlane::new("http://127.0.0.1:1").expect("create test control plane");
        assert!(
            control_plane
                .lookup_storage_resource(
                    "secret",
                    APP_ID.parse().expect("valid application ID"),
                    ResourceKind::Database,
                    "../filesystems/id"
                )
                .await
                .is_err()
        );
        assert!(
            control_plane
                .lookup_storage_resource(
                    "secret",
                    APP_ID.parse().expect("valid application ID"),
                    ResourceKind::Database,
                    ID
                )
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn device_lookup_without_management_owner_cannot_fall_back_to_idp() {
        let control = HostedControlPlane::new("http://127.0.0.1:1").expect("unused IdP");
        assert!(
            control
                .get_storage_device_identity(ID.parse().expect("Device ID"))
                .await
                .is_err()
        );
        let endpoint = iroh::SecretKey::generate().public().to_string();
        assert!(
            control
                .get_storage_endpoint_identity(&endpoint)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn selection_device_lookup_rejects_idp_outage() {
        let control_plane =
            HostedControlPlane::new("http://127.0.0.1:1").expect("create test control plane");
        assert!(
            control_plane
                .get_storage_device_identity(ID.parse().expect("valid device ID"))
                .await
                .is_err()
        );
    }

    #[test]
    fn service_urls_require_tls_except_for_loopback() {
        for url in ["http://localhost/idp", "http://127.0.0.1:3000/idp"] {
            assert!(normalize_service_url(url).is_ok());
        }
        assert!(normalize_service_url("https://idp.example/idp").is_ok());
        for url in [
            "http://idp.example",
            "http://user:secret@localhost/idp",
            "https://idp.example/idp?token=secret",
            "https://idp.example/idp#fragment",
            "ftp://idp.example/idp",
        ] {
            assert!(normalize_service_url(url).is_err(), "accepted {url}");
        }
    }

    #[test]
    fn idp_and_storage_service_urls_keep_independent_prefixes() {
        let control_plane = HostedControlPlane::new_with_services(
            "https://host.example/idp/",
            "https://host.example/storage/",
            "https://issuer.example",
        )
        .expect("create split control plane");

        assert_eq!(
            control_plane
                .idp_base_url
                .join("devices")
                .expect("join IdP route")
                .as_str(),
            "https://host.example/idp/devices"
        );
        assert_eq!(
            control_plane
                .storage_base_url
                .join("databases/00000000-0000-0000-0000-000000000001")
                .expect("join Storage route")
                .as_str(),
            "https://host.example/storage/databases/00000000-0000-0000-0000-000000000001"
        );
    }

    #[test]
    fn issuer_is_independent_of_api_base() {
        let issuer = idp_service::oauth2::OAuth2Config::default().issuer;
        let control_plane = HostedControlPlane::new_with_issuer("https://api.example", &issuer)
            .expect("create control plane with distinct issuer");
        let mut signed_claims = claims();
        signed_claims.iss = issuer.to_owned();
        assert!(control_plane.check_issuer(&signed_claims).is_ok());
        signed_claims.iss = "https://api.example".into();
        assert!(control_plane.check_issuer(&signed_claims).is_err());
        let other_api = HostedControlPlane::new_with_issuer("https://other-api.example", &issuer)
            .expect("create control plane with different API base");
        assert!(other_api.check_issuer(&signed_claims).is_err());
        signed_claims.iss = issuer.to_owned();
        assert!(other_api.check_issuer(&signed_claims).is_ok());
    }

    #[test]
    fn rejects_invalid_issuers() {
        for issuer in [
            "",
            "not-a-url",
            "ftp://idp.example",
            "https://",
            "https://idp.example/?x=1",
            "https://idp.example/#fragment",
            "https://user@idp.example",
        ] {
            assert!(
                HostedControlPlane::new_with_issuer("https://api.example", issuer).is_err(),
                "accepted issuer {issuer}"
            );
        }
        assert!(
            HostedControlPlane::new_with_issuer("ftp://api.example", "https://idp.example")
                .is_err()
        );
    }

    #[test]
    fn accepts_only_http_control_plane_urls() {
        let control_plane = HostedControlPlane::new("https://lidp.example/")
            .expect("create control plane using API URL as issuer");
        let mut signed_claims = claims();
        signed_claims.iss = "https://lidp.example".into();
        assert!(control_plane.check_issuer(&signed_claims).is_ok());
        assert!(HostedControlPlane::new("https://lidp.example").is_ok());
        assert!(HostedControlPlane::new("ftp://lidp.example").is_err());
        assert!(HostedControlPlane::new("https://lidp.example/?x=1").is_err());
    }
}
