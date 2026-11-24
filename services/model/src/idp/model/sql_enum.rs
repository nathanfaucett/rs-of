use serde::{Deserialize, Deserializer, Serializer, de::Error};

use crate::idp::contract::client_profile::ClientProfile;
use crate::idp::contract::client_type::ClientType;
use crate::idp::contract::code_challenge_method::CodeChallengeMethod;
use crate::idp::contract::entity_type::EntityType;
use crate::idp::contract::sex::Sex;
use crate::idp::contract::token_endpoint_auth_method::TokenEndpointAuthMethod;

pub mod client_type {
    use super::{ClientType, Deserialize, Deserializer, Error, Serializer};
    #[cfg(not(feature = "std"))]
    use alloc::format;

    /// Serializes a client type as its persisted integer value.
    ///
    /// # Errors
    /// Returns an error if the serializer rejects the integer.
    #[allow(
        clippy::trivially_copy_pass_by_ref,
        reason = "Serde serialize_with callbacks receive &T"
    )]
    pub fn serialize<S>(value: &ClientType, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i64(*value as i64)
    }

    /// Deserializes a client type from its persisted integer value.
    ///
    /// # Errors
    /// Returns an error if the input is not a supported client type integer.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<ClientType, D::Error>
    where
        D: Deserializer<'de>,
    {
        match i64::deserialize(deserializer)? {
            0 => Ok(ClientType::Confidential),
            1 => Ok(ClientType::Public),
            v => Err(D::Error::custom(format!("invalid ClientType value: {v}"))),
        }
    }
}

pub mod client_profile {
    use super::{ClientProfile, Deserialize, Deserializer, Error, Serializer};
    #[cfg(not(feature = "std"))]
    use alloc::format;

    /// Serializes a client profile as its persisted integer value.
    ///
    /// # Errors
    /// Returns an error if the serializer rejects the integer.
    #[allow(
        clippy::trivially_copy_pass_by_ref,
        reason = "Serde serialize_with callbacks receive &T"
    )]
    pub fn serialize<S>(value: &ClientProfile, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i64(*value as i64)
    }

    /// Deserializes a client profile from its persisted integer value.
    ///
    /// # Errors
    /// Returns an error if the input is not a supported client profile integer.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<ClientProfile, D::Error>
    where
        D: Deserializer<'de>,
    {
        match i64::deserialize(deserializer)? {
            0 => Ok(ClientProfile::Web),
            1 => Ok(ClientProfile::UserAgentBased),
            2 => Ok(ClientProfile::Native),
            v => Err(D::Error::custom(format!(
                "invalid ClientProfile value: {v}"
            ))),
        }
    }
}

pub mod token_endpoint_auth_method {
    use super::{Deserialize, Deserializer, Error, Serializer, TokenEndpointAuthMethod};
    #[cfg(not(feature = "std"))]
    use alloc::format;

    /// Serializes a token endpoint authentication method as its persisted integer.
    ///
    /// # Errors
    /// Returns an error if the serializer rejects the integer.
    #[allow(
        clippy::trivially_copy_pass_by_ref,
        reason = "Serde serialize_with callbacks receive &T"
    )]
    pub fn serialize<S>(value: &TokenEndpointAuthMethod, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i64(*value as i64)
    }

    /// Deserializes a token endpoint authentication method from its persisted integer.
    ///
    /// # Errors
    /// Returns an error if the input is not a supported authentication method integer.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<TokenEndpointAuthMethod, D::Error>
    where
        D: Deserializer<'de>,
    {
        match i64::deserialize(deserializer)? {
            0 => Ok(TokenEndpointAuthMethod::ClientSecretBasic),
            1 => Ok(TokenEndpointAuthMethod::ClientSecretPost),
            2 => Ok(TokenEndpointAuthMethod::PrivateKeyJwt),
            3 => Ok(TokenEndpointAuthMethod::ClientSecretJwt),
            4 => Ok(TokenEndpointAuthMethod::None),
            v => Err(D::Error::custom(format!(
                "invalid TokenEndpointAuthMethod value: {v}"
            ))),
        }
    }
}

pub mod code_challenge_method_option {
    use super::{CodeChallengeMethod, Deserialize, Deserializer, Error, Serializer};
    #[cfg(not(feature = "std"))]
    use alloc::format;

    /// Serializes an optional code challenge method as its persisted integer.
    ///
    /// # Errors
    /// Returns an error if the serializer rejects the value.
    #[allow(
        clippy::ref_option,
        clippy::trivially_copy_pass_by_ref,
        reason = "Serde serialize_with callbacks receive &Option<T>"
    )]
    pub fn serialize<S>(
        value: &Option<CodeChallengeMethod>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(v) => serializer.serialize_some(&(*v as i64)),
            None => serializer.serialize_none(),
        }
    }

    /// Deserializes an optional code challenge method from its persisted integer.
    ///
    /// # Errors
    /// Returns an error if the input is not a supported code challenge method integer.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<CodeChallengeMethod>, D::Error>
    where
        D: Deserializer<'de>,
    {
        match Option::<i64>::deserialize(deserializer)? {
            Some(0) => Ok(Some(CodeChallengeMethod::S256)),
            Some(v) => Err(D::Error::custom(format!(
                "invalid CodeChallengeMethod value: {v}"
            ))),
            None => Ok(None),
        }
    }
}

pub mod entity_type {
    use super::{Deserialize, Deserializer, EntityType, Error, Serializer};
    #[cfg(not(feature = "std"))]
    use alloc::format;

    /// Serializes an entity type as its persisted integer value.
    ///
    /// # Errors
    /// Returns an error if the serializer rejects the integer.
    #[allow(
        clippy::trivially_copy_pass_by_ref,
        reason = "Serde serialize_with callbacks receive &T"
    )]
    pub fn serialize<S>(value: &EntityType, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i64(*value as i64)
    }

    /// Deserializes an entity type from its persisted integer value.
    ///
    /// # Errors
    /// Returns an error if the input is not a supported entity type integer.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<EntityType, D::Error>
    where
        D: Deserializer<'de>,
    {
        match i64::deserialize(deserializer)? {
            0 => Ok(EntityType::User),
            1 => Ok(EntityType::Client),
            v => Err(D::Error::custom(format!("invalid EntityType value: {v}"))),
        }
    }
}

pub mod sex_option {
    use super::{Deserialize, Deserializer, Error, Serializer, Sex};
    #[cfg(not(feature = "std"))]
    use alloc::format;

    /// Serializes an optional sex value as its persisted integer.
    ///
    /// # Errors
    /// Returns an error if the serializer rejects the value.
    #[allow(
        clippy::ref_option,
        clippy::trivially_copy_pass_by_ref,
        reason = "Serde serialize_with callbacks receive &Option<T>"
    )]
    pub fn serialize<S>(value: &Option<Sex>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(v) => serializer.serialize_some(&(*v as i64)),
            None => serializer.serialize_none(),
        }
    }

    /// Deserializes an optional sex value from its persisted integer.
    ///
    /// # Errors
    /// Returns an error if the input is not a supported sex integer.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Sex>, D::Error>
    where
        D: Deserializer<'de>,
    {
        match Option::<i64>::deserialize(deserializer)? {
            Some(0) => Ok(Some(Sex::Male)),
            Some(1) => Ok(Some(Sex::Female)),
            Some(v) => Err(D::Error::custom(format!("invalid Sex value: {v}"))),
            None => Ok(None),
        }
    }
}
