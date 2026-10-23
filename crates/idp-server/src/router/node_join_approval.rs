use std::io;

use db::{Row, SqlTranslator, Transaction, Value};
use device_service::DeviceRepo;
use idp_model::{
    contract::{
        IdentityAction, IdentityResource, IdpSignerRecord, PermissionTarget, ReplicaMembership,
    },
    model::Id,
};
use idp_service::replica::DbReplicaSignerRepo;
use iroh_chain::{MAX_PAIRING_PAYLOAD_LENGTH, PairingOffer};

use super::middleware::require_identity_permission_for_audience;
use crate::{RouterState, authorize_bearer, node_join::invalid};
use device_service::hosted_join::NodeJoinApproval;
use device_service::hosted_join::NodeJoinRequest;

impl RouterState {
    /// Approves only an authenticated pairing offer using a locally authenticated User session.
    /// A successful reply proves enrollment, not bootstrap of all contexts on the joining Node.
    pub async fn authorize_node_join_approver(&self, bearer: &str) -> io::Result<Id> {
        let actor = authorize_bearer(self, bearer).await.map_err(auth_error)?;
        for (action, resource) in [
            (
                IdentityAction::DevicePairingUpdate,
                IdentityResource::DevicePairing,
            ),
            (
                IdentityAction::ReplicaSignersEnroll,
                IdentityResource::ReplicaSigner { member_id: None },
            ),
        ] {
            require_identity_permission_for_audience(
                self,
                &actor,
                management_service::MANAGEMENT_APPLICATION_URI,
                action,
                PermissionTarget::Installation { resource },
            )
            .await
            .map_err(auth_error)?;
        }
        Ok(actor.principal.get_entity_id())
    }

    pub async fn approve_node_join(
        &self,
        offer: &PairingOffer,
        bearer: &str,
    ) -> io::Result<NodeJoinApproval> {
        let approval = self.enroll_node_join_offer(offer, bearer).await?;
        self.reply_node_join(offer, bearer, &approval).await?;
        Ok(approval)
    }

    /// Validates and commits exact owner enrollment without publishing a pairing reply.
    pub async fn enroll_node_join_offer(
        &self,
        offer: &PairingOffer,
        bearer: &str,
    ) -> io::Result<NodeJoinApproval> {
        #[cfg(test)]
        let started = std::time::Instant::now();
        #[cfg(test)]
        eprintln!(
            "IdP join enrollment stage=authorization_begin elapsed={:?}",
            started.elapsed()
        );
        let owner = self.authorize_node_join_approver(bearer).await?;
        #[cfg(test)]
        eprintln!(
            "IdP join enrollment stage=authorization_complete elapsed={:?}",
            started.elapsed()
        );
        let request: NodeJoinRequest = serde_json::from_slice(&offer.payload)?;
        request.validate()?;
        let issuer = self.oauth2_service.metadata().issuer;
        if request.installation != issuer
            || request.endpoint_id != offer.remote_id.to_string()
            || request.approving_peer != self.transport_identity()?.endpoint_id().to_string()
        {
            return Err(invalid(
                "join Installation or authenticated endpoint binding mismatch",
            ));
        }
        let now: i64 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_secs()
            .try_into()
            .map_err(io::Error::other)?;
        self.verify_node_join_approving_device(&request.approving_peer)
            .await?;
        let approval = NodeJoinApproval::signed_by(self.transport_identity()?, request, owner)?;
        let reply = serde_json::to_vec(&approval)?;
        if reply.len() > MAX_PAIRING_PAYLOAD_LENGTH {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "signed join approval exceeds the pairing payload limit",
            ));
        }
        #[cfg(test)]
        if let Some((reached, resume)) = &self.before_node_join_activation {
            reached.notify_one();
            resume.notified().await;
        }
        #[cfg(test)]
        eprintln!(
            "IdP join enrollment stage=transaction_begin elapsed={:?}",
            started.elapsed()
        );
        if self.authorize_node_join_approver(bearer).await? != owner {
            return Err(invalid("enrollment approver differs from approved owner"));
        }
        self.verify_node_join_approving_device(&approval.request.approving_peer)
            .await?;
        let result = self.enroll_node_join(&approval.request, owner, now).await;
        #[cfg(test)]
        eprintln!(
            "IdP join enrollment stage=transaction_returned successful={} elapsed={:?}",
            result.is_ok(),
            started.elapsed()
        );
        result?;
        self.verify_node_join_enrollment(&approval).await?;
        Ok(approval)
    }

    pub async fn verify_node_join_enrollment(&self, approval: &NodeJoinApproval) -> io::Result<()> {
        let request = &approval.request;
        approval.verify(request)?;
        if request.installation != self.oauth2_service.metadata().issuer
            || request.approving_peer != self.transport_identity()?.endpoint_id().to_string()
        {
            return Err(invalid(
                "enrollment is not bound to this Installation and approving endpoint",
            ));
        }
        let device = self
            .management_devices()?
            .find_approved_by_public_key(&request.endpoint_id)
            .await
            .map_err(io::Error::other)?
            .ok_or_else(|| invalid("approved enrollment Device is unavailable"))?;
        if device.id != request.member_id
            || device.owner_subject != approval.owner_subject.to_string()
            || device.name != request.name
        {
            return Err(invalid(
                "approved enrollment Device ownership/binding changed",
            ));
        }
        self.oauth2_service
            .find_user_info(approval.owner_subject)
            .await
            .map_err(auth_error)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_secs()
            .try_into()
            .map_err(io::Error::other)?;
        let trusted = self
            .replica_signers
            .trusted_signers(&request.installation, now)
            .await
            .map_err(io::Error::other)?;
        self.verify_node_join_approving_device(&request.approving_peer)
            .await?;
        if !trusted.into_iter().any(|(member, signer)| {
            member.member_id == request.member_id
                && member.endpoint_id == request.endpoint_id
                && signer.public_jwk == request.public_signer
        }) {
            return Err(invalid(
                "approved enrollment signer/membership is not currently trusted",
            ));
        }
        Ok(())
    }

    async fn verify_node_join_approving_device(&self, endpoint: &str) -> io::Result<()> {
        let device = self
            .management_devices()?
            .find_approved_by_public_key(endpoint)
            .await
            .map_err(io::Error::other)?
            .ok_or_else(|| invalid("approving endpoint has no currently approved Device"))?;
        let owner: Id = device.owner_subject.parse().map_err(io::Error::other)?;
        if owner.is_nil() || owner.to_string() != device.owner_subject {
            return Err(invalid(
                "approving Device owner is not an exact active User",
            ));
        }
        self.oauth2_service
            .find_user_info(owner)
            .await
            .map_err(auth_error)?;
        Ok(())
    }

    /// Rechecks authorization and exact current enrollment immediately before reply publication.
    pub async fn reply_node_join(
        &self,
        offer: &PairingOffer,
        bearer: &str,
        approval: &NodeJoinApproval,
    ) -> io::Result<()> {
        if self.authorize_node_join_approver(bearer).await? != approval.owner_subject {
            return Err(invalid("reply approver differs from enrolled owner"));
        }
        let request: NodeJoinRequest = serde_json::from_slice(&offer.payload)?;
        if offer.remote_id.to_string() != request.endpoint_id || request != approval.request {
            return Err(invalid(
                "pairing reply does not bind the exact enrolled offer",
            ));
        }
        self.verify_node_join_enrollment(approval).await?;
        let reply = serde_json::to_vec(approval)?;
        if reply.len() > MAX_PAIRING_PAYLOAD_LENGTH {
            return Err(invalid("signed join reply exceeds pairing payload limit"));
        }
        offer.reply(&reply).await
    }

    async fn enroll_node_join(
        &self,
        request: &NodeJoinRequest,
        owner: Id,
        now: i64,
    ) -> io::Result<()> {
        self.management_devices()?
            .enroll_approved_exact(
                request.member_id,
                owner,
                &request.name,
                &request.endpoint_id,
                now,
            )
            .await
            .map_err(io::Error::other)?;
        let membership = ReplicaMembership {
            installation_id: request.installation.clone(),
            member_id: request.member_id,
            endpoint_id: request.endpoint_id.clone(),
            issuer: request.installation.clone(),
            approved_at: now,
            revoked_at: None,
        };
        let signer = IdpSignerRecord {
            member_id: request.member_id,
            key_id: request
                .public_signer
                .kid
                .parse()
                .map_err(io::Error::other)?,
            issuer: request.installation.clone(),
            public_jwk: request.public_signer.clone(),
            approved_at: Some(now),
            revoked_at: None,
            expires_at: None,
        };
        let mut transaction = self
            .engine
            .client()
            .transaction()
            .await
            .map_err(io::Error::other)?;
        let result = async {
            persist_join_intent(&mut transaction, request, owner).await?;
            DbReplicaSignerRepo::enroll_in_transaction(&mut transaction, &membership, &signer)
                .await
                .map_err(|error| match error {
                    idp_service::repo::RepoError::InvalidInput(message) => invalid(&message),
                    error => io::Error::other(error),
                })?;
            for (table, query) in [
                (
                    "idp_replica_members",
                    format!(
                        "SELECT id FROM idp_replica_members WHERE member_id = CAST('{}' AS UUID)",
                        request.member_id
                    ),
                ),
                (
                    "idp_replica_signers",
                    format!(
                        "SELECT id FROM idp_replica_signers WHERE active_member_id = CAST('{}' AS UUID)",
                        request.member_id
                    ),
                ),
            ] {
                let results = transaction
                    .translate_and_execute(&query, &SqlTranslator)
                    .await
                    .map_err(io::Error::other)?;
                let [row] = results[0].rows.as_slice() else {
                    return Err(invalid("join enrollment is not unique"));
                };
                require_clear(&transaction, table, row).await?;
            }
            Ok(())
        }
        .await;
        match result {
            Ok(()) => transaction.commit().await.map_err(io::Error::other),
            Err(error) => {
                transaction.rollback().await.map_err(io::Error::other)?;
                Err(error)
            }
        }
    }
}

async fn persist_join_intent(
    transaction: &mut Transaction,
    request: &NodeJoinRequest,
    owner: Id,
) -> io::Result<()> {
    transaction.translate_and_execute(
        "CREATE TABLE IF NOT EXISTS local_node_join_intents (id UUID PRIMARY KEY, binding TEXT NOT NULL)",
        &SqlTranslator,
    ).await.map_err(io::Error::other)?;
    let binding =
        String::from_utf8(NodeJoinApproval::payload(request, owner)?).map_err(io::Error::other)?;
    let rows = transaction
        .translate_and_execute(
            &format!(
                "SELECT binding FROM local_node_join_intents WHERE id = CAST('{}' AS UUID)",
                request.member_id,
            ),
            &SqlTranslator,
        )
        .await
        .map_err(io::Error::other)?;
    match rows[0].rows.as_slice() {
        [] => {
            transaction.translate_and_execute(&format!(
                "INSERT INTO local_node_join_intents (id, binding) VALUES (CAST('{}' AS UUID), {})", request.member_id, text(&binding),
            ), &SqlTranslator).await.map_err(io::Error::other)?;
        }
        [row] if row.values == [Value::Text(binding)] => (),
        _ => {
            return Err(invalid(
                "join request or owner conflicts with persisted approval intent",
            ));
        }
    }
    require_clear(
        transaction,
        "local_node_join_intents",
        &Row::new(vec![Value::Uuid(request.member_id)]),
    )
    .await
}

async fn require_clear(transaction: &Transaction, table: &str, key: &Row) -> io::Result<()> {
    if !transaction
        .row_conflicts(table, key)
        .await
        .map_err(io::Error::other)?
        .is_empty()
    {
        return Err(invalid("join enrollment has unresolved owner conflicts"));
    }
    Ok(())
}

fn text(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
fn auth_error(error: idp_model::contract::ErrorResponse) -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        format!("join authorization denied: {:?}", error.error),
    )
}
