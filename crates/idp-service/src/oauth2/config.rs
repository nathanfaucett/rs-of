#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use idp_model::contract::{
    AuthorizationServerMetadata, CodeChallengeMethod, GrantType, ResponseMode, ResponseType,
    TokenEndpointAuthMethod,
};
use serde::{Deserialize, Serialize};

pub const DEFAULT_ISSUER: &str = "https://api.localhost:1355/idp";
pub const DEFAULT_REQUIRE_PKCE: bool = true;
pub const DEFAULT_TOKEN_TTL_SECS: u64 = 3600;
pub const DEFAULT_REFRESH_TOKEN_TTL_SECS: u64 = 2_592_000;
pub const DEFAULT_AUTHORIZATION_CODE_TTL_SECS: u64 = 600;
pub const DEFAULT_DEVICE_CODE_TTL_SECS: i64 = 600;
pub const DEFAULT_DEVICE_POLL_INTERVAL_SECS: i64 = 5;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OAuth2Config {
    pub issuer: String,
    pub local_signer: Option<super::NodeSignerBinding>,
    pub require_pkce: bool,
    pub token_ttl_secs: u64,
    pub refresh_token_ttl_secs: u64,
    pub authorization_code_ttl_secs: u64,
    pub device_code_ttl_secs: i64,
    pub device_poll_interval_secs: i64,
}

impl Default for OAuth2Config {
    fn default() -> Self {
        Self {
            issuer: DEFAULT_ISSUER.to_string(),
            local_signer: None,
            require_pkce: DEFAULT_REQUIRE_PKCE,
            token_ttl_secs: DEFAULT_TOKEN_TTL_SECS,
            refresh_token_ttl_secs: DEFAULT_REFRESH_TOKEN_TTL_SECS,
            authorization_code_ttl_secs: DEFAULT_AUTHORIZATION_CODE_TTL_SECS,
            device_code_ttl_secs: DEFAULT_DEVICE_CODE_TTL_SECS,
            device_poll_interval_secs: DEFAULT_DEVICE_POLL_INTERVAL_SECS,
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "std"))]
    use alloc::format;

    use super::OAuth2Config;
    #[test]
    fn metadata_uses_public_endpoints_without_changing_canonical_issuer() {
        let issuer = "https://canonical-idp.example/idp/";
        let config = OAuth2Config {
            issuer: issuer.into(),
            ..Default::default()
        };
        for base in [
            "https://unified.localhost:1355/lidp",
            "https://unified.localhost:1355/lidp/",
        ] {
            let metadata = config.to_metadata(base);
            assert_eq!(metadata.issuer, issuer);
            for (endpoint, path) in [
                (metadata.authorization_endpoint, "/oauth2/auth"),
                (metadata.token_endpoint, "/oauth2/token"),
                (metadata.jwks_uri, "/.well-known/jwks.json"),
                (metadata.registration_endpoint, "/oauth2/register"),
                (metadata.userinfo_endpoint, "/userinfo"),
                (metadata.revocation_endpoint, "/oauth2/revoke"),
            ] {
                assert_eq!(
                    endpoint.as_deref(),
                    Some(format!("https://unified.localhost:1355/lidp{path}").as_str())
                );
            }
        }
        assert_eq!(config.issuer, issuer);
    }

    #[test]
    fn rejects_retired_role_configuration() {
        for role in ["replica", "authority", "primary"] {
            assert!(
                serde_json::from_value::<OAuth2Config>(serde_json::json!({"role": role})).is_err()
            );
        }
        assert!(serde_json::from_str::<OAuth2Config>("{}").is_ok());
    }
}

impl OAuth2Config {
    pub fn to_metadata(&self, endpoint_base: &str) -> AuthorizationServerMetadata {
        let endpoint_base = endpoint_base.trim_end_matches('/');

        AuthorizationServerMetadata {
            issuer: self.issuer.clone(),
            authorization_endpoint: Some(format!("{endpoint_base}/oauth2/auth")),
            token_endpoint: Some(format!("{endpoint_base}/oauth2/token")),
            jwks_uri: Some(format!("{endpoint_base}/.well-known/jwks.json")),
            registration_endpoint: Some(format!("{endpoint_base}/oauth2/register")),
            userinfo_endpoint: Some(format!("{endpoint_base}/userinfo")),
            scopes_supported: vec![
                "openid".to_string(),
                "profile".to_string(),
                "email".to_string(),
                "address".to_string(),
                "phone".to_string(),
                "offline_access".to_string(),
                "storage".to_string(),
            ],
            response_types_supported: vec![ResponseType::Code],
            response_modes_supported: vec![ResponseMode::Query, ResponseMode::FormPost],
            grant_types_supported: vec![
                GrantType::AuthorizationCode,
                GrantType::ClientCredentials,
                GrantType::RefreshToken,
                GrantType::TokenExchange,
            ],
            token_endpoint_auth_methods_supported: vec![
                TokenEndpointAuthMethod::ClientSecretBasic,
                TokenEndpointAuthMethod::ClientSecretPost,
                TokenEndpointAuthMethod::None,
            ],
            token_endpoint_auth_signing_alg_values_supported: Vec::new(),
            service_documentation: None,
            ui_locales_supported: Vec::new(),
            op_policy_uri: None,
            op_tos_uri: None,
            revocation_endpoint: Some(format!("{endpoint_base}/oauth2/revoke")),
            revocation_endpoint_auth_methods_supported: vec![
                TokenEndpointAuthMethod::ClientSecretBasic,
                TokenEndpointAuthMethod::ClientSecretPost,
                TokenEndpointAuthMethod::None,
            ],
            revocation_endpoint_auth_signing_alg_values_supported: Vec::new(),
            introspection_endpoint: None,
            introspection_endpoint_auth_methods_supported: Vec::new(),
            introspection_endpoint_auth_signing_alg_values_supported: Vec::new(),
            code_challenge_methods_supported: vec![CodeChallengeMethod::S256],
            signed_metadata: None,
            pushed_authorization_request_endpoint: None,
            dpop_signing_alg_values_supported: Vec::new(),
            require_pkce: self.require_pkce,
        }
    }
}
