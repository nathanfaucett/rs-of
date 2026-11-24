#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

extern crate self as idp_service;

// Shorter namespace aliases for use within this crate
#[cfg(feature = "std")]
pub use of_model as model;
#[cfg(feature = "ofdb")]
pub use ofdb_kv as db_kv;
#[cfg(feature = "ofdb")]
pub use ofdb_sql as db_sql;

pub mod oauth2;
#[cfg(feature = "ofdb")]
pub mod ofdb;
pub mod repo;
pub mod security_sync;
#[cfg(feature = "std")]
pub mod setup_credentials;

pub mod password_config;
pub mod util;

// Re-export commonly used types at crate root
pub use oauth2::AuthorizationServerMetadata;
pub use oauth2::ClientPrincipal;
pub use oauth2::Jwks;
pub use oauth2::NodeSignerBinding;
pub use oauth2::OAuth2Config;
pub use oauth2::OAuth2Service;
pub use oauth2::Principal;
pub use oauth2::UserPrincipal;
pub use password_config::PasswordConfig;
pub use repo::ApplicationRepo;
pub use repo::ClientRepo;
pub use repo::KeyRepo;
pub use repo::KeyService;
pub use repo::OAuth2AuthorizationCodeRepo;
pub use repo::OAuth2RefreshTokenRepo;
pub use repo::OAuth2UserConsentRepo;
#[cfg(feature = "std")]
pub use repo::PrivateKeyKeyringRepo;
pub use repo::PrivateKeyRepo;
pub use repo::RepoError;
pub use repo::RepoResult;
pub use repo::UserRepo;
pub use security_sync::{PUBLIC_IDENTITY_POLICY_TABLES, public_security_tables};
pub use util::{encrypt_password, generate_random_string};
