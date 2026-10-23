use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use idp_model::contract::IntrospectionResponse;
#[cfg(feature = "network")]
use iroh::EndpointId;
use model::contract::{TokenResponse, TokenType};
use of_idp_client::{
    apis::{
        Error,
        configuration::Configuration,
        default_api::{TokenParams, token},
        super_routes_oauth2_introspect_api::{IntrospectParams, introspect},
    },
    models::IntrospectionRequest,
};
use reqwest::{Client, Url, redirect::Policy};

const MAX_RESPONSE_SIZE: usize = 64 * 1024;
const SERVICE_SCOPES: &str = "idp.token.validate";
const RENEWAL_MARGIN: Duration = Duration::from_secs(15);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdpIntrospectionError {
    InvalidToken,
    ServiceUnavailable,
}

#[derive(Clone)]
pub struct IdpClient {
    backend: IdpBackend,
    #[cfg(feature = "network")]
    local_device_policy: Option<(EndpointId, crate::LocalReplicationAuthorizer)>,
}

type InspectToken = Arc<
    dyn Fn(
            String,
        ) -> Pin<
            Box<dyn Future<Output = Result<IntrospectionResponse, IdpIntrospectionError>> + Send>,
        > + Send
        + Sync,
>;
#[derive(Clone)]
enum IdpBackend {
    Local { inspect: InspectToken },
    Http(HttpIdpClient),
}

#[derive(Clone)]
struct HttpIdpClient {
    base_url: Url,
    client_id: String,
    client_secret: String,
    issuer: String,
    audience: String,
    client: Client,
    cached_token: std::sync::Arc<Mutex<Option<CachedToken>>>,
}

struct CachedToken {
    value: String,
    valid_until: SystemTime,
}

impl IdpClient {
    pub fn new(
        base_url: &str,
        client_id: &str,
        client_secret: &str,
        issuer: &str,
        audience: &str,
    ) -> Result<Self, String> {
        if client_id.trim().is_empty() || client_secret.is_empty() || audience.trim().is_empty() {
            return Err("IdP OAuth client ID, secret and service audience are required".to_owned());
        }
        let issuer_url = validate_http_url(issuer, "IdP issuer URL")?;
        let mut base_url = validate_http_url(base_url, "IdP API URL")?;
        if !base_url.path().ends_with('/') {
            let path = format!("{}/", base_url.path());
            base_url.set_path(&path);
        }
        let client = Client::builder()
            .tls_built_in_native_certs(true)
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(10))
            .redirect(Policy::none())
            .build()
            .map_err(|_| "could not create IdP HTTP client".to_owned())?;
        Ok(Self {
            backend: IdpBackend::Http(HttpIdpClient {
                base_url,
                client_id: client_id.to_owned(),
                client_secret: client_secret.to_owned(),
                issuer: issuer_url.to_string().trim_end_matches('/').to_owned(),
                audience: audience.to_owned(),
                client,
                cached_token: Arc::new(Mutex::new(None)),
            }),
            #[cfg(feature = "network")]
            local_device_policy: None,
        })
    }

    /// The callback must query the current local IdP owner on every invocation.
    pub fn local<F, U>(inspect: F) -> Self
    where
        F: Fn(String) -> U + Send + Sync + 'static,
        U: Future<Output = Result<IntrospectionResponse, IdpIntrospectionError>> + Send + 'static,
    {
        Self {
            backend: IdpBackend::Local {
                inspect: Arc::new(move |token| Box::pin(inspect(token))),
            },
            #[cfg(feature = "network")]
            local_device_policy: None,
        }
    }

    #[cfg(feature = "network")]
    pub fn with_local_device_policy(
        mut self,
        endpoint: EndpointId,
        policy: crate::LocalReplicationAuthorizer,
    ) -> Self {
        self.local_device_policy = Some((endpoint, policy));
        self
    }

    pub async fn introspect(
        &self,
        token: &str,
    ) -> Result<IntrospectionResponse, IdpIntrospectionError> {
        #[cfg(feature = "network")]
        if let Some((endpoint, policy)) = &self.local_device_policy {
            let (response, owner) =
                tokio::join!(self.inspect_identity(token), policy.local_owner(*endpoint));
            let response = response?;
            let owner = owner.map_err(|_| IdpIntrospectionError::InvalidToken)?;

            if response.claims.sub != owner {
                return Err(IdpIntrospectionError::InvalidToken);
            }
            return Ok(response);
        }
        self.inspect_identity(token).await
    }

    async fn inspect_identity(
        &self,
        token: &str,
    ) -> Result<IntrospectionResponse, IdpIntrospectionError> {
        match &self.backend {
            IdpBackend::Local { inspect } => inspect(token.to_owned()).await,
            IdpBackend::Http(http) => http.introspect(token).await,
        }
    }
}

impl HttpIdpClient {
    fn configuration(&self, bearer_access_token: Option<String>) -> Configuration {
        Configuration {
            base_path: self.base_url.as_str().trim_end_matches('/').to_owned(),
            client: self.client.clone(),
            response_limit: MAX_RESPONSE_SIZE,
            bearer_access_token,
            ..Configuration::default()
        }
    }

    async fn service_access_token(&self) -> Result<String, String> {
        if let Some(token) = self
            .cached_token
            .lock()
            .map_err(|_| "IdP service token cache is unavailable".to_owned())?
            .as_ref()
            .filter(|token| {
                token
                    .valid_until
                    .duration_since(SystemTime::now())
                    .is_ok_and(|remaining| remaining > RENEWAL_MARGIN)
            })
            .map(|token| token.value.clone())
        {
            return Ok(token);
        }

        let configuration = self.configuration(None);
        let response = token(
            &configuration,
            TokenParams {
                grant_type: Some("client_credentials".to_owned()),
                client_id: Some(self.client_id.clone()),
                client_secret: Some(self.client_secret.clone()),
                scope: Some(SERVICE_SCOPES.to_owned()),
                audience: Some(self.audience.clone()),
                ..Default::default()
            },
        )
        .await
        .map_err(|_| {
            "IdP token endpoint is unavailable or rejected the Storage service client".to_owned()
        })?;
        let token: TokenResponse = serde_json::from_value(
            serde_json::to_value(response).map_err(|_| "invalid IdP token response".to_owned())?,
        )
        .map_err(|_| "invalid IdP token response".to_owned())?;
        if token.token_type != TokenType::Bearer
            || token.issuer.as_deref() != Some(self.issuer.as_str())
            || !token.scope.as_deref().is_some_and(|scope| {
                let granted = scope.split_ascii_whitespace().collect::<Vec<_>>();
                SERVICE_SCOPES
                    .split_ascii_whitespace()
                    .all(|required| granted.contains(&required))
            })
        {
            return Err("IdP returned an unauthorized Storage service token".to_owned());
        }
        let expires_in = token
            .expires_in
            .ok_or_else(|| "IdP service token has no expiry".to_owned())?;
        let valid_until = SystemTime::now()
            .checked_add(Duration::from_secs(expires_in))
            .ok_or_else(|| "IdP service token expiry is invalid".to_owned())?;
        let value = token.access_token.0;
        *self
            .cached_token
            .lock()
            .map_err(|_| "IdP service token cache is unavailable".to_owned())? =
            Some(CachedToken {
                value: value.clone(),
                valid_until,
            });
        Ok(value)
    }

    async fn introspect(
        &self,
        token: &str,
    ) -> Result<IntrospectionResponse, IdpIntrospectionError> {
        let started = std::time::Instant::now();
        let caller_token = self
            .service_access_token()
            .await
            .map_err(|_| IdpIntrospectionError::ServiceUnavailable)?;
        log::debug!(
            "Storage authentication phase=service-token elapsed_ms={}",
            started.elapsed().as_millis()
        );
        let configuration = self.configuration(Some(caller_token));
        let response = introspect(
            &configuration,
            IntrospectParams {
                introspection_request: IntrospectionRequest {
                    token: token.to_owned(),
                    token_type_hint: Some(Some("access_token".to_owned())),
                },
            },
        )
        .await
        .map_err(|error| match error {
            Error::ResponseError(response) if response.status.as_u16() == 401 => {
                IdpIntrospectionError::InvalidToken
            }
            _ => IdpIntrospectionError::ServiceUnavailable,
        })?;
        let response: IntrospectionResponse = serde_json::from_value(
            serde_json::to_value(response)
                .map_err(|_| IdpIntrospectionError::ServiceUnavailable)?,
        )
        .map_err(|_| IdpIntrospectionError::ServiceUnavailable)?;
        log::debug!(
            "Storage authentication phase=introspection elapsed_ms={}",
            started.elapsed().as_millis()
        );
        Ok(response)
    }
}

fn validate_http_url(value: &str, description: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|_| format!("invalid {description}"))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host().is_none()
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || (url.scheme() == "http" && !is_loopback(&url))
    {
        return Err(format!("{description} must use HTTPS except for loopback"));
    }
    Ok(url)
}

fn is_loopback(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    use super::{HttpIdpClient, IdpBackend, IdpClient};

    fn http(client: &IdpClient) -> &HttpIdpClient {
        let IdpBackend::Http(http) = &client.backend else {
            panic!("expected HTTP client");
        };
        http
    }

    #[tokio::test]
    async fn oversized_introspection_request_is_rejected_before_sending_credentials() {
        use axum::{Json, routing::post};
        let app = axum::Router::new()
            .route(
                "/idp/oauth2/token",
                post(|| async {
                    Json(serde_json::json!({
                        "access_token": "service-token", "token_type": "Bearer", "expires_in": 300,
                        "scope": "idp.token.validate", "iss": ISSUER
                    }))
                }),
            )
            .route(
                "/idp/oauth2/introspect",
                post(|| async { axum::http::StatusCode::UNAUTHORIZED }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind IdP listener");
        let base = format!(
            "http://{}/idp",
            listener.local_addr().expect("listener address")
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve IdP");
        });
        let client =
            IdpClient::new(&base, CLIENT_ID, CLIENT_SECRET, ISSUER, AUDIENCE).expect("IdP client");
        assert_eq!(
            client.introspect(&"x".repeat(1024 * 1024)).await.err(),
            Some(super::IdpIntrospectionError::ServiceUnavailable)
        );
        server.abort();
    }

    const CLIENT_ID: &str = "storage-service";
    const CLIENT_SECRET: &str = "test-secret";
    const ISSUER: &str = "https://idp.example";
    const AUDIENCE: &str = "https://idp-api.example";

    #[tokio::test]
    async fn http_introspection_requests_only_token_validation_scope() {
        use axum::{
            Json,
            http::{HeaderMap, StatusCode},
            routing::post,
        };

        let app = axum::Router::new()
            .route("/idp/oauth2/token", post(|body: String| async move {
                assert!(body.contains("scope=idp.token.validate"));
                assert!(!body.contains("idp.device"));
                Json(serde_json::json!({
                    "access_token": "service-token", "token_type": "Bearer", "expires_in": 300,
                    "scope": "idp.token.validate", "iss": ISSUER
                }))
            }))
            .route("/idp/oauth2/introspect", post(|headers: HeaderMap, Json(request): Json<idp_model::contract::IntrospectionRequest>| async move {
                assert_eq!(headers["authorization"], "Bearer service-token");
                assert_eq!(request.token, "invalid-user-token");
                StatusCode::UNAUTHORIZED
            }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind IdP API");
        let base = format!("http://{}", listener.local_addr().expect("IdP API address"));
        let server =
            tokio::spawn(async move { axum::serve(listener, app).await.expect("serve IdP API") });
        let client = IdpClient::new(
            &format!("{base}/idp"),
            CLIENT_ID,
            CLIENT_SECRET,
            ISSUER,
            AUDIENCE,
        )
        .expect("inspect-only IdP client");
        assert!(matches!(
            client.introspect("invalid-user-token").await,
            Err(super::IdpIntrospectionError::InvalidToken)
        ));
        server.abort();
        let _ = server.await;
    }

    #[cfg(feature = "network")]
    #[tokio::test]
    async fn local_introspection_checks_current_device_owner_without_endpoint_enumeration() {
        use std::sync::{Arc, Mutex};

        use device_service::DeviceState;
        use idp_model::{contract::IntrospectionResponse, model::Id};
        use model::contract::{PrincipalType, StandardClaims, TokenType, TokenUse};

        let endpoint = iroh::SecretKey::generate().public();
        let owner = Id::now_v7().to_string();
        let response = IntrospectionResponse {
            claims: StandardClaims {
                r#type: TokenType::Bearer,
                r#use: TokenUse::Access,
                exp: i64::MAX,
                iat: 0,
                nbf: 0,
                iss: ISSUER.into(),
                aud: "https://storage.example".into(),
                client_id: "application-client".into(),
                sub: owner.clone(),
                principal_type: PrincipalType::User,
                resource: None,
                authorization_details: None,
                scope: vec![],
            },
            application_id: Id::now_v7().to_string(),
        };
        let client = IdpClient::local(move |token| {
            assert_eq!(token, "user-token");
            let response = response.clone();
            async move { Ok(response) }
        });
        assert_eq!(
            client
                .introspect("user-token")
                .await
                .expect("inspect-only owner call")
                .claims
                .sub,
            owner
        );
        let evidence = Arc::new(Mutex::new(crate::DeviceReplicationPolicy {
            device_id: Id::now_v7(),
            endpoint_id: endpoint,
            owner_subject: owner.clone(),
            state: DeviceState::Approved,
            owner_active: true,
            admin_allowed: true,
            resources: vec![],
        }));
        let current = evidence.clone();
        let policy = crate::LocalReplicationAuthorizer::new(move |_| {
            let policy = current.lock().expect("current owner evidence").clone();
            async move { Ok(policy) }
        });
        let client = client.with_local_device_policy(endpoint, policy);
        assert_eq!(
            client
                .introspect("user-token")
                .await
                .expect("approved owner with empty selection")
                .claims
                .sub,
            owner
        );
        let baseline = evidence.lock().expect("approved policy").clone();
        for denial in 0..5 {
            let mut policy = baseline.clone();
            match denial {
                0 => policy.owner_subject = Id::now_v7().to_string(),
                1 => policy.state = DeviceState::Revoked,
                2 => policy.owner_active = false,
                3 => policy.admin_allowed = false,
                _ => policy.endpoint_id = iroh::SecretKey::generate().public(),
            }
            *evidence.lock().expect("learn current owner denial") = policy;
            assert!(matches!(
                client.introspect("user-token").await,
                Err(super::IdpIntrospectionError::InvalidToken)
            ));
        }
        *evidence.lock().expect("restore current owner") = baseline;
        assert!(client.introspect("user-token").await.is_ok());
    }

    #[test]
    fn validates_service_urls_and_client_configuration() {
        assert!(
            IdpClient::new(
                "http://127.0.0.1:3000/idp",
                CLIENT_ID,
                CLIENT_SECRET,
                ISSUER,
                AUDIENCE
            )
            .is_ok()
        );
        assert!(
            IdpClient::new(
                "https://idp.example/idp",
                CLIENT_ID,
                CLIENT_SECRET,
                ISSUER,
                AUDIENCE
            )
            .is_ok()
        );
        assert!(
            IdpClient::new(
                "http://idp.example",
                CLIENT_ID,
                CLIENT_SECRET,
                ISSUER,
                AUDIENCE
            )
            .is_err()
        );
        assert!(
            IdpClient::new("https://idp.example", "", CLIENT_SECRET, ISSUER, AUDIENCE).is_err()
        );
        assert!(
            IdpClient::new(
                "https://idp.example",
                CLIENT_ID,
                CLIENT_SECRET,
                "http://issuer.example",
                AUDIENCE
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn reacquires_expired_service_token() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test HTTP server");
        let address = listener.local_addr().expect("read test HTTP address");
        let server = thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().expect("accept token request");
                let mut request = [0; 4096];
                let length = stream.read(&mut request).expect("read token request");
                let request = String::from_utf8_lossy(&request[..length]).into_owned();
                assert!(request.starts_with("POST /idp/oauth2/token HTTP/1.1"));
                let body = r#"{"access_token":"service-token","token_type":"Bearer","expires_in":0,"scope":"idp.token.validate","iss":"https://idp.example"}"#;
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).expect("write token response");
            }
        });
        let client = IdpClient::new(
            &format!("http://{address}/idp"),
            CLIENT_ID,
            CLIENT_SECRET,
            ISSUER,
            AUDIENCE,
        )
        .expect("create test IdP client");

        assert_eq!(
            http(&client)
                .service_access_token()
                .await
                .expect("first token"),
            "service-token"
        );
        assert_eq!(
            http(&client)
                .service_access_token()
                .await
                .expect("renewed token"),
            "service-token"
        );
        server.join().expect("join test HTTP server");
    }

    #[tokio::test]
    async fn rejects_token_endpoint_redirects() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind token endpoint");
        let address = listener.local_addr().expect("read token endpoint address");
        let redirect_listener = TcpListener::bind("127.0.0.1:0").expect("bind redirect target");
        redirect_listener
            .set_nonblocking(true)
            .expect("make redirect target nonblocking");
        let redirect_address = redirect_listener
            .local_addr()
            .expect("read redirect target address");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept token request");
            let mut request = [0; 4096];
            stream.read(&mut request).expect("read token request");
            write!(
                stream,
                "HTTP/1.1 302 Found\r\nLocation: http://{redirect_address}/stolen\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )
            .expect("write redirect response");
        });
        let client = IdpClient::new(
            &format!("http://{address}/idp"),
            CLIENT_ID,
            CLIENT_SECRET,
            ISSUER,
            AUDIENCE,
        )
        .expect("create test IdP client");

        assert!(http(&client).service_access_token().await.is_err());
        server.join().expect("join test HTTP server");
        assert!(matches!(
            redirect_listener.accept(),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
        ));
    }

    #[tokio::test]
    async fn rejects_oversized_token_response_without_caching() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test HTTP server");
        let address = listener.local_addr().expect("read test HTTP address");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept token request");
            let mut request = [0; 4096];
            stream.read(&mut request).expect("read token request");
            let mut body = r#"{"access_token":"service-token","token_type":"Bearer","expires_in":3600,"scope":"idp.token.validate","iss":"https://idp.example"}"#.to_owned();
            body.push_str(&" ".repeat(65_537 - body.len()));
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
                .expect("write oversized token response");
        });
        let client = IdpClient::new(
            &format!("http://{address}/idp"),
            CLIENT_ID,
            CLIENT_SECRET,
            ISSUER,
            AUDIENCE,
        )
        .expect("create test IdP client");

        assert!(http(&client).service_access_token().await.is_err());
        assert!(
            http(&client)
                .cached_token
                .lock()
                .expect("lock token cache")
                .is_none()
        );
        server.join().expect("join test HTTP server");
    }

    #[tokio::test]
    async fn rejects_service_token_acquisition_without_caching() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test HTTP server");
        let address = listener.local_addr().expect("read test HTTP address");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept token request");
            let mut request = [0; 4096];
            stream.read(&mut request).expect("read token request");
            let body = r#"{"error":"invalid_client"}"#;
            write!(stream, "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).expect("write token rejection");
        });
        let client = IdpClient::new(
            &format!("http://{address}/idp"),
            CLIENT_ID,
            CLIENT_SECRET,
            ISSUER,
            AUDIENCE,
        )
        .expect("create test IdP client");

        assert!(http(&client).service_access_token().await.is_err());
        assert!(
            http(&client)
                .cached_token
                .lock()
                .expect("lock token cache")
                .is_none()
        );
        server.join().expect("join test HTTP server");
    }

    #[test]
    fn retains_path_prefix_for_normal_api_and_token_urls() {
        let client = IdpClient::new(
            "http://127.0.0.1:3000/idp",
            CLIENT_ID,
            CLIENT_SECRET,
            ISSUER,
            AUDIENCE,
        )
        .expect("valid client configuration");
        assert_eq!(
            http(&client)
                .base_url
                .join("oauth2/token")
                .expect("join token endpoint")
                .path(),
            "/idp/oauth2/token"
        );
        assert_eq!(
            http(&client)
                .base_url
                .join("oauth2/introspect")
                .expect("join introspection endpoint")
                .path(),
            "/idp/oauth2/introspect"
        );
    }
}
