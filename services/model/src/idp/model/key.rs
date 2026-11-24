use core::{fmt::Write, str::FromStr};

#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};

use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use bip32::DerivationPath;
use chrono::{DateTime, Utc};
use key::derived_key::DerivedKey;
use key::error::KeyResult;

use crate::id::Id;
use crate::idp::contract::entity_type::EntityType;
use crate::idp::contract::jwk_private::JwkPrivate;
use crate::idp::contract::jwk_private_parameters::JwkPrivateParameters;
use crate::idp::contract::jwk_public::JwkPublic;
use crate::idp::contract::jwk_public_parameters::JwkPublicParameters;
use crate::idp::contract::jws_algorithm::JwsAlgorithm;
use crate::idp::contract::key_use::KeyUse;
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Key {
    pub id: Id,

    pub parent_id: Option<Id>,

    #[serde(with = "super::sql_enum::entity_type")]
    pub entity_type: EntityType,
    pub entity_id: Id,

    #[serde(deserialize_with = "crate::idp::model::none_to_default::none_to_default")]
    pub derivation_path: String,
    pub derivation_index: u32,
    pub name: String,
    pub hardened: bool,
    pub public_jwk: Option<JwkPublic>,

    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub revoked_at: Option<DateTime<Utc>>,
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub expires_at: Option<DateTime<Utc>>,

    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub updated_at: DateTime<Utc>,
}

impl Key {
    /// Converts the derived key to a private JWK.
    ///
    /// # Errors
    /// Returns a key error if the private JWK components cannot be derived.
    pub fn to_jwk_private(&self, derived_key: &DerivedKey) -> KeyResult<JwkPrivate> {
        let (x, y, d) = derived_key.to_xyd()?;

        let jwt = JwkPrivate {
            r#use: KeyUse::Signature,
            kid: self.id.to_string(),
            alg: JwsAlgorithm::ES256K,
            params: JwkPrivateParameters::Ec {
                crv: "secp256k1".to_string(),
                x: STANDARD_NO_PAD.encode(x),
                y: STANDARD_NO_PAD.encode(y),
                d: STANDARD_NO_PAD.encode(d),
            },
        };

        Ok(jwt)
    }

    /// Converts the derived key to a public JWK.
    ///
    /// # Errors
    /// Returns a key error if the public JWK components cannot be derived.
    pub fn to_jwk_public(&self, derived_key: &DerivedKey) -> KeyResult<JwkPublic> {
        let (x, y) = derived_key.to_xy()?;

        let jwt = JwkPublic {
            r#use: KeyUse::Signature,
            kid: self.id.to_string(),
            alg: JwsAlgorithm::ES256K,
            params: JwkPublicParameters::Ec {
                crv: "secp256k1".to_string(),
                x: STANDARD_NO_PAD.encode(x),
                y: STANDARD_NO_PAD.encode(y),
            },
        };

        Ok(jwt)
    }

    /// Parses the stored BIP32 derivation path.
    ///
    /// # Errors
    /// Returns a key error if the stored derivation path is invalid.
    pub fn derivation_path(&self) -> KeyResult<DerivationPath> {
        let derivation_path = DerivationPath::from_str(&self.derivation_path)?;
        Ok(derivation_path)
    }

    #[must_use]
    pub fn build_derivation_path(
        parent_derivation_path: Option<&str>,
        derivation_index: u32,
        hardened: bool,
    ) -> String {
        let mut path = String::new();

        if let Some(parent_path) = parent_derivation_path {
            path.push_str(parent_path);
        } else {
            path.push('m');
        }

        path.push('/');
        write!(path, "{derivation_index}").expect("writing to a String cannot fail");
        if hardened {
            path.push('\'');
        }

        path
    }
}

#[cfg(test)]
mod tests {
    use core::str::FromStr;

    use bip32::DerivationPath;

    use crate::idp::model::key::Key;

    #[test]
    fn builds_valid_bip32_paths_from_sibling_indices() {
        let root = Key::build_derivation_path(None, 0, true);
        let child = Key::build_derivation_path(Some(&root), 1, false);

        assert_eq!(root, "m/0'");
        assert_eq!(child, "m/0'/1");
        assert_ne!(
            Key::build_derivation_path(None, 0, false),
            Key::build_derivation_path(None, 1, false)
        );
        DerivationPath::from_str(&child).expect("generated path must be valid");
    }
}
