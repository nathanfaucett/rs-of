use std::io;

use db::Client;
use idp_model::{contract::JwkPublic, model::Id};

use super::{DbReplicaSignerRepo, DbUserRepo};
use crate::{PasswordConfig, oauth2::NodeSignerBinding, repo::UserRepo};

pub struct LocalIdentity {
    engine: Client,
}

impl LocalIdentity {
    pub const fn new(engine: Client) -> Self {
        Self { engine }
    }

    pub async fn is_active_user(&self, subject: Id) -> io::Result<bool> {
        if subject.is_nil() {
            return Ok(false);
        }
        Ok(
            DbUserRepo::new(self.engine.clone(), PasswordConfig::default())
                .find_user_by_id(subject)
                .await
                .map_err(denied)?
                .is_some(),
        )
    }

    pub async fn check_join_identity(
        &self,
        installation: &str,
        member: Id,
        endpoint: &str,
        public_signer: &JwkPublic,
        owner: Id,
        now: i64,
    ) -> io::Result<()> {
        let key: Id = public_signer.kid.parse().map_err(denied)?;
        let trusted = DbReplicaSignerRepo::new(self.engine.clone())
            .trusted_signers(installation, now)
            .await
            .map_err(denied)?;
        let mut matching = trusted.iter().filter(|(identity, signer)| {
            identity.member_id == member
                && identity.endpoint_id == endpoint
                && signer.key_id == key
                && signer.public_jwk == *public_signer
        });
        matching.next().ok_or_else(|| {
            denied("bootstrap requires exact current enrolled member and independent signer")
        })?;
        if matching.next().is_some() {
            return Err(denied(
                "bootstrap requires exact current enrolled member and independent signer",
            ));
        }
        if !self.is_active_user(owner).await? {
            return Err(denied(
                "bootstrap owner is not a current conflict-clear User",
            ));
        }
        Ok(())
    }

    pub fn check_configured_join_signer(
        configured: Option<&NodeSignerBinding>,
        member: Id,
        endpoint: &str,
        public_signer: &JwkPublic,
    ) -> io::Result<()> {
        let key: Id = public_signer.kid.parse().map_err(denied)?;
        if !configured.is_some_and(|binding| {
            binding.member_id == member && binding.key_id == key && binding.endpoint_id == endpoint
        }) {
            return Err(denied("local independent signing capability is not ready"));
        }
        Ok(())
    }

    pub async fn check_signer_binding(
        &self,
        installation: &str,
        binding: &NodeSignerBinding,
        now: i64,
    ) -> io::Result<()> {
        let trusted = DbReplicaSignerRepo::new(self.engine.clone())
            .trusted_signers(installation, now)
            .await
            .map_err(denied)?;
        if !trusted.iter().any(|(member, signer)| {
            member.member_id == binding.member_id
                && member.endpoint_id == binding.endpoint_id
                && signer.key_id == binding.key_id
        }) {
            return Err(denied("local node signer/member is not currently trusted"));
        }
        Ok(())
    }
}

fn denied(reason: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, reason.to_string())
}
