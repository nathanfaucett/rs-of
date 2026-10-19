use std::io;

use idp_model::{
    contract::{
        ApplicationRegistration, ClientProfile, ClientRegistration, ClientType, GrantType,
        TokenEndpointAuthMethod,
    },
    model::Id,
};
use serde::{Deserialize, Serialize};

use crate::repo::RawKeyringRepo;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SetupRelationship {
    IdpManagement,
    ManagementIdp,
    StorageIdp,
    StorageManagement,
}

impl SetupRelationship {
    pub fn owner(self) -> &'static str {
        match self {
            Self::IdpManagement => "idp",
            Self::ManagementIdp => "management",
            Self::StorageIdp | Self::StorageManagement => "storage",
        }
    }
    pub fn registration(
        self,
        id: Id,
        idp: &str,
        management: &str,
        storage: &str,
    ) -> ClientRegistration {
        let (consumer, name, scopes, audience): (&str, &str, &[&str], &str) = match self {
            Self::IdpManagement => (
                idp,
                "IdP permission evaluation",
                &["management.permission.evaluate"],
                "idp-management",
            ),
            Self::ManagementIdp => (
                management,
                "Management identity validation",
                &["idp.token.validate", "idp.user.lookup"],
                idp,
            ),
            Self::StorageIdp => (
                storage,
                "Storage identity validation",
                &["idp.token.validate"],
                idp,
            ),
            Self::StorageManagement => (
                storage,
                "Storage current endpoint policy",
                &[
                    "management.device.list",
                    "management.replication.read",
                    "management.replication.admit",
                ],
                storage,
            ),
        };
        ClientRegistration {
            application: ApplicationRegistration {
                name: Some(format!("Independent {}", self.owner())),
                uri: format!("{consumer}/infrastructure"),
                description: None,
            },
            client_id: Some(id.to_string()),
            client_secret: None,
            client_id_issued_at: None,
            client_secret_expires_at: None,
            client_name: name.into(),
            client_uri: Some(format!("{consumer}/infrastructure")),
            logo_uri: None,
            contacts: Vec::new(),
            terms_of_service_uri: None,
            policy_uri: None,
            client_type: ClientType::Confidential,
            profile: ClientProfile::Web,
            redirect_uris: Vec::new(),
            allowed_grant_types: vec![GrantType::ClientCredentials],
            response_types: Vec::new(),
            allowed_scopes: scopes.iter().map(|scope| (*scope).into()).collect(),
            allowed_audiences: match self {
                Self::ManagementIdp => vec![
                    idp.into(),
                    "idp-management".into(),
                    storage.into(),
                    "storage:databases".into(),
                    "storage:filesystems".into(),
                ],
                Self::StorageIdp => vec![
                    idp.into(),
                    "storage:databases".into(),
                    "storage:filesystems".into(),
                ],
                _ => vec![audience.into()],
            },
            token_endpoint_auth_method: if self == Self::IdpManagement {
                TokenEndpointAuthMethod::ClientSecretBasic
            } else {
                TokenEndpointAuthMethod::ClientSecretPost
            },
            software_statement: None,
            software_id: None,
            software_version: None,
        }
    }
}

/// Call only after persisting an owner-local attempted-material fence for this exact slot.
pub fn create_setup_secret(service: &str, slot: &str) -> io::Result<()> {
    let provider = RawKeyringRepo::new(service).map_err(io::Error::other)?;
    if provider
        .load("", "", slot)
        .map_err(io::Error::other)?
        .is_some()
    {
        return Err(io::Error::other("unexpected protected setup material"));
    }
    let mut random = [0u8; 32];
    getrandom::fill(&mut random).map_err(|_| io::Error::other("secure randomness unavailable"))?;
    let secret: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
    provider
        .store("", "", slot, secret.as_bytes())
        .map_err(io::Error::other)?;
    if load_setup_secret(service, slot)? != secret {
        return Err(io::Error::other(
            "protected setup material readback disagrees",
        ));
    }
    Ok(())
}

pub fn load_setup_secret(service: &str, slot: &str) -> io::Result<String> {
    let bytes = RawKeyringRepo::new(service)
        .map_err(io::Error::other)?
        .load("", "", slot)
        .map_err(io::Error::other)?
        .ok_or_else(|| io::Error::other("attempted protected setup material is missing"))?;
    let value = String::from_utf8(bytes)
        .map_err(|_| io::Error::other("protected setup material is malformed"))?;
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(io::Error::other("protected setup material is malformed"));
    }
    Ok(value)
}
