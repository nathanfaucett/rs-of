use of_model::id::Id;
use of_model::idp::contract::application_registration::ApplicationRegistration;
use of_model::idp::contract::client_profile::ClientProfile;
use of_model::idp::contract::client_registration::ClientRegistration;
use of_model::idp::contract::client_type::ClientType;
use of_model::idp::contract::grant_type::GrantType;
use of_model::idp::contract::permission_evaluation::MANAGEMENT_POLICY_ADMINISTRATION_SCOPE;
use of_model::idp::contract::token_endpoint_auth_method::TokenEndpointAuthMethod;

pub fn operator_login_registration(
    id: Id,
    idp: &str,
    device: &str,
    storage: &str,
) -> ClientRegistration {
    ClientRegistration {
        application: ApplicationRegistration {
            name: Some("Local Node operator".into()),
            uri: idp.into(),
            description: None,
        },
        client_id: Some(id.to_string()),
        client_secret: None,
        client_id_issued_at: None,
        client_secret_expires_at: None,
        client_name: "Local Node operator".into(),
        client_uri: None,
        logo_uri: None,
        contacts: Vec::new(),
        terms_of_service_uri: None,
        policy_uri: None,
        client_type: ClientType::Public,
        profile: ClientProfile::Native,
        redirect_uris: Vec::new(),
        allowed_grant_types: vec![
            GrantType::Password,
            GrantType::RefreshToken,
            GrantType::TokenExchange,
        ],
        response_types: Vec::new(),
        allowed_scopes: vec![
            "openid".into(),
            "profile".into(),
            "email".into(),
            MANAGEMENT_POLICY_ADMINISTRATION_SCOPE.into(),
        ],
        allowed_audiences: vec![
            idp.into(),
            "idp-management".into(),
            device.into(),
            storage.into(),
            "storage:databases".into(),
            "storage:filesystems".into(),
        ],
        token_endpoint_auth_method: TokenEndpointAuthMethod::None,
        software_statement: None,
        software_id: None,
        software_version: None,
    }
}
