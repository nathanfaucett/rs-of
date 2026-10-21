use std::io;

use idp_model::{
    contract::{
        ClientCredentialsGrantRequest, ErrorCode, ErrorResponse,
        MANAGEMENT_PERMISSION_EVALUATE_SCOPE, OAuth2ClientAuth, PermissionEvaluationRequest,
        PermissionEvaluationResponse, PermissionSubject, TokenEndpointAuthMethod, TokenRequest,
    },
    model::Id,
};
use model::contract::PrincipalType;
use of_management_client::apis::{
    Error,
    configuration::Configuration,
    default_api::{EvaluatePermissionParams, evaluate_permission},
};
use reqwest::{Client, Url};

use crate::{ManagementPermissionConfig, RouterState};

#[derive(Clone)]
pub struct ManagementPermissionClient {
    base: Url,
    client_id: String,
    client_secret: String,
    client: Client,
}

impl ManagementPermissionClient {
    pub fn new(config: &ManagementPermissionConfig) -> io::Result<Self> {
        let mut base = Url::parse(&config.api_base_uri).map_err(|_| invalid_configuration())?;
        if base.scheme() != "https"
            || base.host().is_none()
            || !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
            || config.client_id.trim().is_empty()
            || config.client_secret.trim().is_empty()
        {
            return Err(invalid_configuration());
        }
        base.set_path(&format!("{}/", base.path().trim_end_matches('/')));
        Ok(Self {
            base,
            client_id: config.client_id.clone(),
            client_secret: config.client_secret.clone(),
            client: api::LocalTls::verification_client()?,
        })
    }

    pub async fn evaluate(
        &self,
        state: &RouterState,
        request: &PermissionEvaluationRequest,
    ) -> Result<bool, ErrorResponse> {
        let PermissionSubject::User { id } = request.subject;
        if id.is_nil() || request.request_id.is_nil() || request.policy_namespace().is_none() {
            return Err(ErrorResponse::new(ErrorCode::AccessDenied));
        }
        let token = state
            .oauth2_service
            .token(
                TokenRequest::ClientCredentials(ClientCredentialsGrantRequest {
                    client_id: self.client_id.clone(),
                    client_secret: self.client_secret.clone(),
                    scope: Some(MANAGEMENT_PERMISSION_EVALUATE_SCOPE.into()),
                    audience: Some(management_service::MANAGEMENT_APPLICATION_URI.into()),
                    resource: None,
                }),
                Some(OAuth2ClientAuth {
                    client_id: self.client_id.clone(),
                    client_secret: Some(self.client_secret.clone()),
                    method: TokenEndpointAuthMethod::ClientSecretBasic,
                }),
            )
            .await
            .map_err(|_| unavailable())?;
        let claims = state
            .oauth2_service
            .validate_access_token(
                &token.access_token.0,
                management_service::MANAGEMENT_APPLICATION_URI,
            )
            .await
            .map_err(|_| unavailable())?;
        let service_subject = claims.sub.parse::<Id>().map_err(|_| unavailable())?;
        if claims.principal_type != PrincipalType::Client
            || claims.client_id != self.client_id
            || claims.scope != [MANAGEMENT_PERMISSION_EVALUATE_SCOPE]
            || service_subject.is_nil()
        {
            return Err(unavailable());
        }
        let configuration = Configuration {
            base_path: self.base.as_str().trim_end_matches('/').to_owned(),
            client: self.client.clone(),
            bearer_access_token: Some(token.access_token.0),
            ..Default::default()
        };
        let response = evaluate_permission(
            &configuration,
            EvaluatePermissionParams {
                permission_evaluation_request: serde_json::from_value(
                    serde_json::to_value(request).map_err(|_| unavailable())?,
                )
                .map_err(|_| unavailable())?,
            },
        )
        .await
        .map_err(|error| match error {
            Error::ResponseError(response) if matches!(response.status.as_u16(), 401 | 403) => {
                ErrorResponse::new(ErrorCode::AccessDenied)
            }
            _ => unavailable(),
        })?;
        let decision: PermissionEvaluationResponse =
            serde_json::from_value(serde_json::to_value(response).map_err(|_| unavailable())?)
                .map_err(|_| unavailable())?;
        if decision.request != *request
            || decision.audit.actor != request.subject
            || decision.audit.service_client_id != self.client_id
            || decision.audit.service_subject != service_subject
        {
            return Err(unavailable());
        }
        Ok(decision.allowed)
    }

    #[cfg(test)]
    pub(crate) fn with_test_root(mut self, pem: &[u8]) -> io::Result<Self> {
        self.client = Client::builder()
            .use_rustls_tls()
            .tls_built_in_root_certs(false)
            .add_root_certificate(reqwest::Certificate::from_pem(pem).map_err(io::Error::other)?)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(std::time::Duration::from_secs(1))
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(io::Error::other)?;
        Ok(self)
    }
}

fn unavailable() -> ErrorResponse {
    ErrorResponse::new(ErrorCode::TemporarilyUnavailable)
}

fn invalid_configuration() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "Management permission evaluation requires an HTTPS base URI without credentials, query or fragment and an evaluator Client credential",
    )
}
