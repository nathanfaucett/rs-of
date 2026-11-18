//! Replica modules for `IdP` service
//!
//! This module re-exports the database repositories from the `ofdb` module
//! to provide a stable `idp_service::replica` path for downstream crates.

pub use crate::ofdb::application_repo::DbApplicationRepo;
pub use crate::ofdb::client_repo::DbClientRepo;
pub use crate::ofdb::key_repo::DbKeyRepo;
pub use crate::ofdb::local_identity::LocalIdentity;
pub use crate::ofdb::oauth2_authorization_code_repo::DbOAuth2AuthorizationCodeRepo;
pub use crate::ofdb::oauth2_refresh_token_repo::DbOAuth2RefreshTokenRepo;
pub use crate::ofdb::oauth2_user_consent_repo::DbOAuth2UserConsentRepo;
pub use crate::ofdb::replica_signer_repo::DbReplicaSignerRepo;
pub use crate::ofdb::user_repo::DbUserRepo;
