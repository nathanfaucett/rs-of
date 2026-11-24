use alloc::format;
#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};

use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use bip32::DerivationPath;
use key::derived_key::DerivedKey;
use of_model::id::Id;
use of_model::idp::contract::jwk_private::JwkPrivate;
use of_model::idp::contract::jwk_private_parameters::JwkPrivateParameters;
use of_model::idp::contract::jwk_public::JwkPublic;
use of_model::idp::contract::jws_algorithm::JwsAlgorithm;
use of_model::idp::contract::key_use::KeyUse;

use crate::repo::{KeyRepo, KeyService, PrivateKeyRepo, RepoError, RepoResult};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_field_names,
    reason = "field names are part of the persisted signer-binding contract"
)]
pub struct NodeSignerBinding {
    pub member_id: Id,
    pub key_id: Id,
    pub endpoint_id: String,
}

impl<R: KeyRepo, P: PrivateKeyRepo> KeyService<R, P> {
    fn node_namespace(&self, binding: &NodeSignerBinding) -> String {
        format!(
            "{}:node:{}:{}",
            self.namespace(),
            binding.member_id,
            binding.key_id
        )
    }

    /// # Errors
    /// Returns [`RepoError`] if the binding is incomplete or key preparation fails.
    pub fn prepare_node_signer(&self, binding: &NodeSignerBinding) -> RepoResult<JwkPublic> {
        if binding.member_id.is_nil()
            || binding.key_id.is_nil()
            || binding.endpoint_id.trim().is_empty()
        {
            return Err(RepoError::InvalidInput(
                "node signer binding is incomplete".into(),
            ));
        }
        let private = self
            .private_key_repo()
            .ensure_master_key(&self.node_namespace(binding))?;
        Ok(private_jwk(binding, &private)?.into())
    }

    /// # Errors
    /// Returns [`RepoError`] if trusted key state or local private key material is invalid.
    #[allow(
        clippy::future_not_send,
        reason = "repository futures intentionally support single-threaded no_std executors"
    )]
    pub async fn load_node_signer(
        &self,
        binding: &NodeSignerBinding,
        issuer: &str,
        now: i64,
    ) -> RepoResult<JwkPrivate> {
        let (public, _) = self
            .key_repo()
            .issuer_keys(issuer, now)
            .await?
            .into_iter()
            .find(|(_, active)| *active)
            .ok_or_else(|| RepoError::InvalidInput("local issuer key is not active".into()))?;
        let mut binding = binding.clone();
        binding.key_id = public.kid.parse().map_err(RepoError::other)?;
        let private = self
            .private_key_repo()
            .load(&self.node_namespace(&binding), &DerivationPath::default())?
            .ok_or_else(|| {
                RepoError::InvalidInput("local node private signer is missing".into())
            })?;
        let jwk = private_jwk(&binding, &private)?;
        if JwkPublic::from(jwk.clone()) != public {
            return Err(RepoError::InvalidInput(
                "local node private signer does not match approved public material".into(),
            ));
        }
        Ok(jwk)
    }
}

fn private_jwk(binding: &NodeSignerBinding, private: &DerivedKey) -> RepoResult<JwkPrivate> {
    let (x, y, d) = private.to_xyd()?;
    Ok(JwkPrivate {
        kid: binding.key_id.to_string(),
        r#use: KeyUse::Signature,
        alg: JwsAlgorithm::ES256K,
        params: JwkPrivateParameters::Ec {
            crv: "secp256k1".into(),
            x: STANDARD_NO_PAD.encode(x),
            y: STANDARD_NO_PAD.encode(y),
            d: STANDARD_NO_PAD.encode(d),
        },
    })
}
