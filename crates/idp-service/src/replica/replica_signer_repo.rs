use db::{QueryError, SqlTranslator, Transaction, Value};
use idp_model::contract::{IdpSignerRecord, JwsAlgorithm, KeyUse, ReplicaMembership};

use crate::repo::{RepoError, RepoResult};

pub struct DbReplicaSignerRepo {
    engine: db::Client,
}

impl DbReplicaSignerRepo {
    pub fn validate_public_key(public: &idp_model::contract::JwkPublic) -> RepoResult<()> {
        if matches!(&public.params, idp_model::contract::JwkPublicParameters::Ec { crv, .. } if crv == "secp256k1")
            && public.r#use == KeyUse::Signature
            && public.alg == JwsAlgorithm::ES256K
            && crate::oauth2::verifing_key_from_jwt(public).is_ok()
        {
            Ok(())
        } else {
            Err(RepoError::InvalidInput(
                "invalid public token signer".into(),
            ))
        }
    }
    #[must_use]
    pub const fn new(engine: db::Client) -> Self {
        Self { engine }
    }

    /// Read current, unambiguous, conflict-clear IdP token-signing trust, not endpoint approval.
    pub async fn trusted_signers(
        &self,
        issuer: &str,
        now: i64,
    ) -> RepoResult<Vec<(ReplicaMembership, IdpSignerRecord)>> {
        let mut transaction = self.engine.transaction().await.map_err(db_error)?;
        let result = Self::trusted_signers_in(&mut transaction, issuer, now).await;
        let rollback = transaction.rollback().await.map_err(db_error);
        result.and_then(|trusted| rollback.map(|()| trusted))
    }

    async fn trusted_signers_in(
        transaction: &mut Transaction,
        issuer: &str,
        now: i64,
    ) -> RepoResult<Vec<(ReplicaMembership, IdpSignerRecord)>> {
        let mut results = transaction.translate_and_execute(
            "SELECT id, installation_id, member_id, endpoint_id, issuer, approved_at, revoked_at FROM idp_replica_members; SELECT id, member_id, active_member_id, key_id, issuer, public_jwk, approved_at, revoked_at, expires_at FROM idp_replica_signers", &SqlTranslator,
        ).await.map_err(db_error)?;
        if results.len() != 2 {
            return Err(RepoError::InvalidInput(
                "incomplete node trust results".into(),
            ));
        }
        let signers = results.pop().expect("two query results").rows;
        let members = results.pop().expect("one query result").rows;
        let mut candidates = Vec::new();
        let mut pairs = Vec::new();
        for row in signers {
            let [
                Value::Uuid(row_id),
                Value::Uuid(member_id),
                Value::Uuid(active_member_id),
                Value::Uuid(key_id),
                Value::Text(record_issuer),
                Value::Text(public_jwk),
                Value::Integer(approved_at),
                Value::Null,
                expires,
            ] = row.values.as_slice()
            else {
                continue;
            };
            if active_member_id != member_id {
                continue;
            }
            let expires_at = match expires {
                Value::Null => None,
                Value::Integer(value) => Some(*value),
                _ => continue,
            };
            let signer = IdpSignerRecord {
                member_id: *member_id,
                key_id: *key_id,
                issuer: record_issuer.clone(),
                public_jwk: serde_json::from_str(public_jwk).map_err(RepoError::other)?,
                approved_at: Some(*approved_at),
                revoked_at: None,
                expires_at,
            };
            if !signer.matches(*member_id, *key_id, issuer, now)
                || crate::oauth2::verifing_key_from_jwt(&signer.public_jwk).is_err()
            {
                continue;
            }
            for member_row in &members {
                let [
                    Value::Uuid(member_row_id),
                    Value::Text(installation_id),
                    Value::Uuid(id),
                    Value::Text(endpoint_id),
                    Value::Text(member_issuer),
                    Value::Integer(member_approved),
                    Value::Null,
                ] = member_row.values.as_slice()
                else {
                    continue;
                };
                if id != member_id || *member_approved < 0 || *member_approved > now {
                    continue;
                }
                let member = ReplicaMembership {
                    installation_id: installation_id.clone(),
                    member_id: *id,
                    endpoint_id: endpoint_id.clone(),
                    issuer: member_issuer.clone(),
                    approved_at: *member_approved,
                    revoked_at: None,
                };
                if !member.matches(issuer, *member_id, endpoint_id) {
                    continue;
                }
                candidates.push((member, signer.clone()));
                pairs.push((*row_id, *member_row_id));
            }
        }
        let mut clearance = Vec::with_capacity(pairs.len());
        for (signer, _) in &pairs {
            let clear = transaction
                .row_conflicts(
                    "idp_replica_signers",
                    &db::Row::new(vec![Value::Uuid(*signer)]),
                )
                .await
                .map_err(db_error)?
                .is_empty();
            clearance.push(clear);
        }
        for ((_, member), clear) in pairs.iter().zip(&mut clearance) {
            let member_clear = transaction
                .row_conflicts(
                    "idp_replica_members",
                    &db::Row::new(vec![Value::Uuid(*member)]),
                )
                .await
                .map_err(db_error)?
                .is_empty();
            *clear &= member_clear;
        }
        let trusted = candidates
            .into_iter()
            .zip(clearance)
            .filter_map(|(pair, clear)| clear.then_some(pair))
            .collect::<Vec<_>>();
        let ambiguous = trusted.iter().any(|(member, signer)| {
            trusted
                .iter()
                .filter(|(other_member, other_signer)| {
                    other_member.member_id == member.member_id
                        || other_member.endpoint_id == member.endpoint_id
                        || other_signer.key_id == signer.key_id
                })
                .count()
                != 1
        });
        if ambiguous {
            return Err(RepoError::InvalidInput(
                "ambiguous node trust bindings".into(),
            ));
        }
        Ok(trusted)
    }

    /// Store membership and its first approved signer as one unit. Returns `false` for an exact retry.
    pub async fn enroll(
        &self,
        membership: &ReplicaMembership,
        signer: &IdpSignerRecord,
    ) -> RepoResult<bool> {
        let mut transaction = self.engine.transaction().await.map_err(db_error)?;
        let operation = Self::enroll_in_transaction(&mut transaction, membership, signer).await;
        match operation {
            Ok(created) => {
                transaction.commit().await.map_err(db_error)?;
                Ok(created)
            }
            Err(error) => {
                transaction.rollback().await.map_err(db_error)?;
                Err(error)
            }
        }
    }

    /// Enroll in the caller's IdP owner transaction without committing it.
    /// The caller must roll back the transaction on any error.
    pub async fn enroll_in_transaction(
        transaction: &mut Transaction,
        membership: &ReplicaMembership,
        signer: &IdpSignerRecord,
    ) -> RepoResult<bool> {
        validate(membership, signer)?;
        let member_query = format!(
            "SELECT endpoint_id, issuer, approved_at, revoked_at FROM idp_replica_members WHERE installation_id = {} AND member_id = CAST('{}' AS UUID)",
            text(&membership.installation_id),
            membership.member_id,
        );
        let endpoint_query = format!(
            "SELECT member_id FROM idp_replica_members WHERE installation_id = {} AND endpoint_id = {}",
            text(&membership.installation_id),
            text(&membership.endpoint_id),
        );
        let signer_query = format!(
            "SELECT member_id, issuer, public_jwk, approved_at, revoked_at, expires_at FROM idp_replica_signers WHERE key_id = CAST('{}' AS UUID)",
            signer.key_id,
        );
        let active_signer_query = format!(
            "SELECT key_id FROM idp_replica_signers WHERE active_member_id = CAST('{}' AS UUID)",
            membership.member_id,
        );
        let mut query_results = transaction
            .translate_and_execute(
                &format!("{member_query}; {endpoint_query}; {signer_query}; {active_signer_query}"),
                &SqlTranslator,
            )
            .await
            .map_err(db_error)?;
        if query_results.len() != 4 {
            return Err(RepoError::InvalidInput(
                "incomplete replica signer query results".into(),
            ));
        }
        let member = query_results.remove(0).rows.into_iter().next();
        let endpoint = query_results.remove(0).rows.into_iter().next();
        let signer_row = query_results.remove(0).rows.into_iter().next();
        let active_signer = query_results.remove(0).rows.into_iter().next();

        if let (Some(member), Some(signer_row)) = (member.as_ref(), signer_row.as_ref())
                && endpoint.as_ref().is_some_and(|row| {
                    matches!(row.values.first(), Some(Value::Uuid(id)) if *id == membership.member_id)
                }) && active_signer.as_ref().is_some_and(|row| {
                    matches!(row.values.first(), Some(Value::Uuid(id)) if *id == signer.key_id)
                }) && member_matches(member, membership)
                    && signer_matches(signer_row, signer)?
                {
                return Ok(false);
            }
        if member.is_some() || endpoint.is_some() || signer_row.is_some() || active_signer.is_some()
        {
            return Err(RepoError::InvalidInput(
                "replica member, endpoint, or signer conflicts with approved state".into(),
            ));
        }

        let jwk = serde_json::to_string(&signer.public_jwk).map_err(RepoError::other)?;
        let insert_member = format!(
            "INSERT INTO idp_replica_members (id, installation_id, member_id, endpoint_id, issuer, approved_at, revoked_at) VALUES (CAST('{}' AS UUID), {}, CAST('{}' AS UUID), {}, {}, {}, NULL)",
            db::Uuid::now_v7(),
            text(&membership.installation_id),
            membership.member_id,
            text(&membership.endpoint_id),
            text(&membership.issuer),
            membership.approved_at,
        );
        let insert_signer = format!(
            "INSERT INTO idp_replica_signers (id, member_id, active_member_id, key_id, issuer, public_jwk, approved_at, revoked_at, expires_at) VALUES (CAST('{}' AS UUID), CAST('{}' AS UUID), CAST('{}' AS UUID), CAST('{}' AS UUID), {}, {}, {}, NULL, {})",
            db::Uuid::now_v7(),
            signer.member_id,
            signer.member_id,
            signer.key_id,
            text(&signer.issuer),
            text(&jwk),
            signer.approved_at.expect("validated approval timestamp"),
            signer
                .expires_at
                .map_or_else(|| "NULL".to_owned(), |expires| expires.to_string()),
        );
        transaction
            .translate_and_execute(&format!("{insert_member}; {insert_signer}"), &SqlTranslator)
            .await
            .map_err(db_error)?;
        Ok(true)
    }

    pub async fn rotate(
        &self,
        member_id: idp_model::model::Id,
        issuer: &str,
        signer: &IdpSignerRecord,
    ) -> RepoResult<bool> {
        let approved_at = validate_signer(signer, member_id, issuer)?;
        let mut transaction = self.engine.transaction().await.map_err(db_error)?;
        let operation = async {
            let mut results = transaction
                .translate_and_execute(
                    &format!(
                        "SELECT id FROM idp_replica_members WHERE installation_id = {} AND member_id = CAST('{}' AS UUID) AND revoked_at IS NULL; SELECT member_id, issuer, public_jwk, approved_at, revoked_at, expires_at FROM idp_replica_signers WHERE key_id = CAST('{}' AS UUID); SELECT key_id FROM idp_replica_signers WHERE active_member_id = CAST('{}' AS UUID)",
                        text(issuer), member_id, signer.key_id, member_id,
                    ),
                    &SqlTranslator,
                )
                .await
                .map_err(db_error)?;
            if results.len() != 3 {
                return Err(RepoError::InvalidInput("incomplete replica signer query results".into()));
            }
            let member_exists = !results.remove(0).rows.is_empty();
            let replacement = results.remove(0).rows.into_iter().next();
            let active = results.remove(0).rows.into_iter().next();
            if !member_exists {
                return Err(RepoError::InvalidInput("replica membership is not active".into()));
            }
            if let Some(active) = active.as_ref()
                && matches!(active.values.first(), Some(Value::Uuid(id)) if *id == signer.key_id)
                && let Some(replacement) = replacement.as_ref()
                && signer_matches(replacement, signer)?
            {
                return Ok(false);
            }
            if replacement.is_some() || active.is_none() {
                return Err(RepoError::InvalidInput("replacement signer conflicts with active state".into()));
            }
            transaction
                .translate_and_execute(
                    &format!(
                        "UPDATE idp_replica_signers SET revoked_at = {approved_at}, active_member_id = NULL WHERE active_member_id = CAST('{}' AS UUID)",
                        member_id,
                    ),
                    &SqlTranslator,
                )
                .await
                .map_err(db_error)?;
            let jwk = serde_json::to_string(&signer.public_jwk).map_err(RepoError::other)?;
            transaction
                .translate_and_execute(
                    &format!(
                        "INSERT INTO idp_replica_signers (id, member_id, active_member_id, key_id, issuer, public_jwk, approved_at, revoked_at, expires_at) VALUES (CAST('{}' AS UUID), CAST('{}' AS UUID), CAST('{}' AS UUID), CAST('{}' AS UUID), {}, {}, {}, NULL, {})",
                        db::Uuid::now_v7(), signer.member_id, signer.member_id, signer.key_id, text(&signer.issuer), text(&jwk), approved_at,
                        signer.expires_at.map_or_else(|| "NULL".to_owned(), |expires| expires.to_string()),
                    ),
                    &SqlTranslator,
                )
                .await
                .map_err(db_error)?;
            Ok(true)
        }
        .await;
        match operation {
            Ok(changed) => {
                transaction.commit().await.map_err(db_error)?;
                Ok(changed)
            }
            Err(error) => {
                transaction.rollback().await.map_err(db_error)?;
                Err(error)
            }
        }
    }

    pub async fn revoke(
        &self,
        member_id: idp_model::model::Id,
        issuer: &str,
        revoked_at: i64,
    ) -> RepoResult<bool> {
        let mut transaction = self.engine.transaction().await.map_err(db_error)?;
        let operation = async {
            let mut results = transaction
                .translate_and_execute(
                    &format!(
                        "SELECT id FROM idp_replica_members WHERE installation_id = {} AND member_id = CAST('{}' AS UUID) AND revoked_at IS NULL; SELECT key_id FROM idp_replica_signers WHERE active_member_id = CAST('{}' AS UUID)",
                        text(issuer), member_id, member_id,
                    ),
                    &SqlTranslator,
                )
                .await
                .map_err(db_error)?;
            if results.len() != 2 {
                return Err(RepoError::InvalidInput("incomplete replica signer query results".into()));
            }
            let member_exists = !results.remove(0).rows.is_empty();
            let active_exists = !results.remove(0).rows.is_empty();
            if !member_exists || !active_exists {
                return Ok(false);
            }
            transaction
                .translate_and_execute(
                    &format!(
                        "UPDATE idp_replica_signers SET revoked_at = {revoked_at}, active_member_id = NULL WHERE active_member_id = CAST('{}' AS UUID); UPDATE idp_replica_members SET revoked_at = {revoked_at} WHERE installation_id = {} AND member_id = CAST('{}' AS UUID) AND revoked_at IS NULL",
                        member_id, text(issuer), member_id,
                    ),
                    &SqlTranslator,
                )
                .await
                .map_err(db_error)?;
            Ok(true)
        }
        .await;
        match operation {
            Ok(changed) => {
                transaction.commit().await.map_err(db_error)?;
                Ok(changed)
            }
            Err(error) => {
                transaction.rollback().await.map_err(db_error)?;
                Err(error)
            }
        }
    }
}

fn validate(membership: &ReplicaMembership, signer: &IdpSignerRecord) -> RepoResult<()> {
    let approved_at = validate_signer(signer, membership.member_id, &membership.issuer)?;
    if membership.installation_id != membership.issuer
        || !membership.matches(
            &membership.installation_id,
            membership.member_id,
            &membership.endpoint_id,
        )
        || membership.approved_at != approved_at
    {
        return Err(RepoError::InvalidInput(
            "replica membership and signer binding is invalid".into(),
        ));
    }
    Ok(())
}

fn validate_signer(
    signer: &IdpSignerRecord,
    member_id: idp_model::model::Id,
    issuer: &str,
) -> RepoResult<i64> {
    let approved_at = signer
        .approved_at
        .ok_or_else(|| RepoError::InvalidInput("signer approval is required".into()))?;
    let valid_public_key = DbReplicaSignerRepo::validate_public_key(&signer.public_jwk).is_ok();
    if signer.member_id != member_id
        || signer.issuer != issuer
        || signer.revoked_at.is_some()
        || signer.key_id.is_nil()
        || signer.public_jwk.kid != signer.key_id.to_string()
        || signer
            .expires_at
            .is_some_and(|expires| expires <= approved_at)
        || !valid_public_key
    {
        return Err(RepoError::InvalidInput(
            "replica signer binding or public key is invalid".into(),
        ));
    }
    Ok(approved_at)
}

fn member_matches(row: &db::Row, membership: &ReplicaMembership) -> bool {
    matches!(row.values.as_slice(), [Value::Text(endpoint), Value::Text(issuer), Value::Integer(_), Value::Null]
        if endpoint == &membership.endpoint_id && issuer == &membership.issuer)
}

fn signer_matches(row: &db::Row, signer: &IdpSignerRecord) -> RepoResult<bool> {
    let jwk = serde_json::to_string(&signer.public_jwk).map_err(RepoError::other)?;
    Ok(
        matches!(row.values.as_slice(), [Value::Uuid(member), Value::Text(issuer), Value::Text(saved_jwk), Value::Integer(_), Value::Null, expires]
        if *member == signer.member_id
            && issuer == &signer.issuer
            && saved_jwk == &jwk
            && match (expires, signer.expires_at) {
                (Value::Null, None) => true,
                (Value::Integer(saved), Some(expected)) => *saved == expected,
                _ => false,
            }),
    )
}

fn text(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn db_error(error: QueryError) -> RepoError {
    RepoError::other(error)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
    use db::{SqlTranslator, Value};
    use k256::ecdsa::SigningKey;

    use idp_model::{
        contract::{
            IdpSignerRecord, JwkPublic, JwkPublicParameters, JwsAlgorithm, KeyUse,
            ReplicaMembership,
        },
        model::Id,
        replica,
    };

    use super::DbReplicaSignerRepo;

    fn records() -> (ReplicaMembership, IdpSignerRecord) {
        let member_id = Id::from_u128(1);
        let key_id = Id::from_u128(2);
        let issuer = "https://installation.example".to_owned();
        let signing_key = SigningKey::from_slice(&[1_u8; 32])
            .expect("fixed test scalar is a valid secp256k1 key");
        let point = signing_key.verifying_key().to_encoded_point(false);
        let membership = ReplicaMembership {
            installation_id: issuer.clone(),
            member_id,
            endpoint_id: "endpoint-key".to_owned(),
            issuer: issuer.clone(),
            approved_at: 10,
            revoked_at: None,
        };
        let signer = IdpSignerRecord {
            member_id,
            key_id,
            issuer,
            public_jwk: JwkPublic {
                kid: key_id.to_string(),
                r#use: KeyUse::Signature,
                alg: JwsAlgorithm::ES256K,
                params: JwkPublicParameters::Ec {
                    crv: "secp256k1".to_owned(),
                    x: STANDARD_NO_PAD.encode(point.x().expect("public point has x coordinate")),
                    y: STANDARD_NO_PAD.encode(point.y().expect("public point has y coordinate")),
                },
            },
            approved_at: Some(10),
            revoked_at: None,
            expires_at: None,
        };
        (membership, signer)
    }

    async fn public_trust_fixture() -> (Arc<db::Database>, ReplicaMembership, IdpSignerRecord) {
        let engine = Arc::new(db::Database::in_memory());
        replica::up(&engine)
            .await
            .expect("initialize public IdP schema");
        let (mut member, mut signer) = records();
        member.member_id = Id::now_v7();
        signer.member_id = member.member_id;
        signer.key_id = Id::now_v7();
        signer.public_jwk.kid = signer.key_id.to_string();
        DbReplicaSignerRepo::new(engine.client())
            .enroll(&member, &signer)
            .await
            .expect("enroll public Node trust");
        (engine, member, signer)
    }

    #[tokio::test]
    async fn local_identity_requires_exact_enrollment_active_user_and_current_trust() {
        use crate::replica::LocalIdentity;
        let (engine, member, signer) = public_trust_fixture().await;
        let identity = LocalIdentity::new(engine.client());
        let owner = Id::now_v7();
        let check = || {
            identity.check_join_identity(
                &member.issuer,
                member.member_id,
                &member.endpoint_id,
                &signer.public_jwk,
                owner,
                10,
            )
        };
        assert!(!identity.is_active_user(owner).await.expect("missing User"));
        let error = check()
            .await
            .expect_err("signer trust alone cannot establish the owner User");
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        assert_eq!(
            error.to_string(),
            "bootstrap owner is not a current conflict-clear User"
        );
        execute(&engine, &format!("INSERT INTO users (id, handle, name, created_at, updated_at) VALUES (CAST('{owner}' AS UUID), 'owner', 'Owner', 1, 1)")).await;
        assert!(
            identity
                .is_active_user(owner)
                .await
                .expect("current active User")
        );
        assert!(
            !identity
                .is_active_user(Id::nil())
                .await
                .expect("nil is not a User")
        );
        check().await.expect("exact current enrollment");
        let binding = crate::oauth2::NodeSignerBinding {
            member_id: member.member_id,
            key_id: signer.key_id,
            endpoint_id: member.endpoint_id.clone(),
        };
        identity
            .check_signer_binding(&member.issuer, &binding, 10)
            .await
            .expect("current local signer");
        LocalIdentity::check_configured_join_signer(
            Some(&binding),
            member.member_id,
            &member.endpoint_id,
            &signer.public_jwk,
        )
        .expect("exact configured signer");
        assert_eq!(
            LocalIdentity::check_configured_join_signer(
                None,
                member.member_id,
                &member.endpoint_id,
                &signer.public_jwk
            )
            .expect_err("missing configured signer")
            .to_string(),
            "local independent signing capability is not ready"
        );
        for changed in [
            crate::oauth2::NodeSignerBinding {
                member_id: Id::now_v7(),
                ..binding.clone()
            },
            crate::oauth2::NodeSignerBinding {
                key_id: Id::now_v7(),
                ..binding.clone()
            },
            crate::oauth2::NodeSignerBinding {
                endpoint_id: "other-endpoint".into(),
                ..binding.clone()
            },
        ] {
            assert_eq!(
                identity
                    .check_signer_binding(&member.issuer, &changed, 10)
                    .await
                    .expect_err("changed local signer")
                    .to_string(),
                "local node signer/member is not currently trusted"
            );
            assert_eq!(
                LocalIdentity::check_configured_join_signer(
                    Some(&changed),
                    member.member_id,
                    &member.endpoint_id,
                    &signer.public_jwk
                )
                .expect_err("configured signer changed")
                .to_string(),
                "local independent signing capability is not ready"
            );
        }
        for (member_id, endpoint, public) in [
            (
                Id::now_v7(),
                member.endpoint_id.clone(),
                signer.public_jwk.clone(),
            ),
            (
                member.member_id,
                "other-endpoint".into(),
                signer.public_jwk.clone(),
            ),
            (member.member_id, member.endpoint_id.clone(), {
                let mut public = signer.public_jwk.clone();
                public.kid = Id::now_v7().to_string();
                public
            }),
        ] {
            let error = identity
                .check_join_identity(&member.issuer, member_id, &endpoint, &public, owner, 10)
                .await
                .expect_err("changed exact binding denies");
            assert_eq!(
                error.to_string(),
                "bootstrap requires exact current enrolled member and independent signer"
            );
        }
        execute(&engine, "DELETE FROM users").await;
        assert!(
            !identity
                .is_active_user(owner)
                .await
                .expect("learned User deletion")
        );
        assert!(
            check().await.is_err(),
            "existing owner facade rechecks current User state"
        );
        DbReplicaSignerRepo::new(engine.client())
            .revoke(member.member_id, &member.issuer, 10)
            .await
            .expect("learned member revocation");
        assert_eq!(
            check()
                .await
                .expect_err("revocation denies before missing User")
                .to_string(),
            "bootstrap requires exact current enrolled member and independent signer"
        );
        assert!(
            identity
                .check_signer_binding(&member.issuer, &binding, 10)
                .await
                .is_err(),
            "local signer authorization also rechecks revocation"
        );
    }

    #[tokio::test]
    async fn trusted_signers_without_devices_and_key_repo_preserve_pairs() {
        use crate::{replica::DbKeyRepo, repo::KeyRepo};

        let (engine, member, signer) = public_trust_fixture().await;
        assert!(
            engine
                .client()
                .translate_and_execute("SELECT * FROM local_device_enrollments", &SqlTranslator)
                .await
                .is_err(),
            "public trust needs no private enrollment table"
        );
        let trusted = DbReplicaSignerRepo::new(engine.client())
            .trusted_signers(&member.issuer, 10)
            .await
            .expect("current public trust");
        assert_eq!(trusted, vec![(member.clone(), signer.clone())]);
        assert_eq!(
            DbKeyRepo::new(engine.client())
                .trusted_node_signers(&member.issuer, 10)
                .await
                .expect("unchanged pair facade"),
            vec![(member, signer)]
        );
    }

    async fn execute(engine: &db::Database, sql: &str) {
        engine
            .client()
            .translate_and_execute(sql, &SqlTranslator)
            .await
            .expect("change public trust fixture");
    }

    async fn legacy_devices(engine: &db::Database) {
        execute(engine, "CREATE TABLE devices (id UUID PRIMARY KEY, owner_subject TEXT, name TEXT, public_key TEXT, address TEXT, state INTEGER, approved_at INTEGER, revoked_at INTEGER, created_at INTEGER, updated_at INTEGER); CREATE UNIQUE INDEX devices_public_key ON devices (public_key)").await;
    }

    #[tokio::test]
    async fn trusted_signers_preserve_independent_pairs_and_recheck_revocation() {
        {
            let (engine, member, signer) = public_trust_fixture().await;
            let repo = DbReplicaSignerRepo::new(engine.client());
            let mut other_member = member.clone();
            other_member.member_id = Id::now_v7();
            other_member.endpoint_id = "other-endpoint".into();
            let mut other_signer = signer.clone();
            other_signer.member_id = other_member.member_id;
            other_signer.key_id = Id::now_v7();
            other_signer.public_jwk.kid = other_signer.key_id.to_string();
            repo.enroll(&other_member, &other_signer)
                .await
                .expect("enroll independent signer pair");
            assert_eq!(
                repo.trusted_signers(&member.issuer, 12)
                    .await
                    .expect("both pairs start trusted")
                    .len(),
                2
            );
            repo.revoke(member.member_id, &member.issuer, 13)
                .await
                .expect("revoke first pair");
            assert_eq!(
                repo.trusted_signers(&member.issuer, 13)
                    .await
                    .expect("unrelated clear pair remains trusted"),
                vec![(other_member.clone(), other_signer)]
            );
            repo.revoke(other_member.member_id, &member.issuer, 13)
                .await
                .expect("revoke surviving pair");
            assert!(
                repo.trusted_signers(&member.issuer, 13)
                    .await
                    .expect("same repo reads current revocation")
                    .is_empty(),
                "no trust evidence survives the operation"
            );
        }
    }

    #[tokio::test]
    async fn trusted_signers_ignore_malformed_revoked_and_duplicate_devices() {
        let (engine, member, signer) = public_trust_fixture().await;
        legacy_devices(&engine).await;
        let repo = DbReplicaSignerRepo::new(engine.client());
        let expected = vec![(member.clone(), signer)];
        let devices = [
            (member.member_id, "endpoint-key"),
            (Id::now_v7(), "unrelated-endpoint-key"),
        ];
        for (id, endpoint) in devices {
            execute(&engine, &format!(
                "INSERT INTO devices (id, owner_subject, name, public_key, address, state, approved_at, revoked_at, created_at, updated_at) VALUES (CAST('{id}' AS UUID), 'owner', 'Device', '{endpoint}', 'local', 1, 1, NULL, 1, 2)"
            )).await;
        }
        for assignment in [
            "owner_subject = '  '",
            "name = NULL",
            "public_key = NULL",
            "address = NULL",
            "state = 9",
            "approved_at = 'invalid'",
            "revoked_at = 'invalid'",
            "created_at = NULL",
            "updated_at = 'invalid'",
            "state = 0",
            "state = 2, revoked_at = 1",
            "state = 1, revoked_at = 11",
            "created_at = 9223372036854775807",
        ] {
            for (id, _) in devices {
                for (reset_id, endpoint) in devices {
                    execute(&engine, &format!(
                        "UPDATE devices SET owner_subject = 'owner', name = 'Device', public_key = '{endpoint}', address = 'local', state = 1, approved_at = 1, revoked_at = NULL, created_at = 1, updated_at = 2 WHERE id = CAST('{reset_id}' AS UUID)"
                    )).await;
                }
                execute(
                    &engine,
                    &format!("UPDATE devices SET {assignment} WHERE id = CAST('{id}' AS UUID)"),
                )
                .await;
                assert_eq!(
                    repo.trusted_signers(&member.issuer, 10)
                        .await
                        .expect("Device state cannot revoke IdP signer trust"),
                    expected,
                    "device={id}, {assignment}"
                );
            }
        }
        let (engine, member, signer) = public_trust_fixture().await;
        legacy_devices(&engine).await;
        let repo = DbReplicaSignerRepo::new(engine.client());
        let expected = vec![(member.clone(), signer)];
        execute(&engine, "DROP INDEX devices_public_key").await;
        for state in [0, 1, 2] {
            execute(&engine, &format!(
                "INSERT INTO devices (id, owner_subject, public_key, state) VALUES (CAST('{}' AS UUID), 'owner', 'endpoint-key', {state})", Id::now_v7()
            )).await;
        }
        assert_eq!(
            repo.trusted_signers(&member.issuer, 10)
                .await
                .expect("duplicate Device endpoints are not IdP signer ambiguity"),
            expected
        );
    }

    #[tokio::test]
    async fn trusted_signers_need_no_device_tables() {
        let (engine, member, signer) = public_trust_fixture().await;
        for table in ["local_device_enrollments", "devices"] {
            assert!(
                engine
                    .client()
                    .translate_and_execute(&format!("SELECT * FROM {table}"), &SqlTranslator)
                    .await
                    .is_err(),
                "{table} is absent from IdP"
            );
        }
        assert_eq!(
            DbReplicaSignerRepo::new(engine.client())
                .trusted_signers(&member.issuer, 10)
                .await
                .expect("IdP trust reads only member and signer records"),
            vec![(member, signer)]
        );
    }

    #[tokio::test]
    async fn trusted_signers_require_exact_active_member_and_signer_bindings() {
        for (table, assignment) in [
            (
                "idp_replica_members",
                "installation_id = 'https://other.example'",
            ),
            ("idp_replica_members", "issuer = 'https://other.example'"),
            (
                "idp_replica_members",
                "member_id = CAST('00000000-0000-0000-0000-000000000003' AS UUID)",
            ),
            ("idp_replica_members", "endpoint_id = ''"),
            ("idp_replica_members", "endpoint_id = ' endpoint-key'"),
            ("idp_replica_members", "revoked_at = 11"),
            ("idp_replica_members", "approved_at = 11"),
            ("idp_replica_members", "approved_at = NULL"),
            ("idp_replica_members", "approved_at = 'invalid'"),
            ("idp_replica_signers", "issuer = 'https://other.example'"),
            (
                "idp_replica_signers",
                "member_id = CAST('00000000-0000-0000-0000-000000000003' AS UUID)",
            ),
            ("idp_replica_signers", "active_member_id = NULL"),
            (
                "idp_replica_signers",
                "active_member_id = CAST('00000000-0000-0000-0000-000000000003' AS UUID)",
            ),
            (
                "idp_replica_signers",
                "key_id = CAST('00000000-0000-0000-0000-000000000003' AS UUID)",
            ),
            ("idp_replica_signers", "revoked_at = 11"),
            ("idp_replica_signers", "approved_at = 11"),
            ("idp_replica_signers", "approved_at = NULL"),
            ("idp_replica_signers", "approved_at = 'invalid'"),
            ("idp_replica_signers", "expires_at = 10"),
            ("idp_replica_signers", "expires_at = 'invalid'"),
        ] {
            let (engine, member, _) = public_trust_fixture().await;
            execute(&engine, &format!("UPDATE {table} SET {assignment}")).await;
            assert!(
                DbReplicaSignerRepo::new(engine.client())
                    .trusted_signers(&member.issuer, 10)
                    .await
                    .expect("invalid IdP binding is excluded")
                    .is_empty(),
                "{table}: {assignment}"
            );
        }
        for (table, column) in [
            ("idp_replica_members", "approved_at"),
            ("idp_replica_signers", "approved_at"),
            ("idp_replica_signers", "expires_at"),
        ] {
            let (engine, member, _) = public_trust_fixture().await;
            engine
                .client()
                .execute(vec![db::Statement::Query(db::Query::Update(
                    db::QueryUpdate {
                        from: db::QueryFrom {
                            table: table.into(),
                            joins: vec![],
                        },
                        assignments: vec![db::QueryUpdateAssignment {
                            column: db::QueryColumn::new(table.into(), column.into()),
                            value: db::QueryExprValue::Value(Value::Integer(-1)),
                        }],
                        predicate: None,
                        returning: None,
                    },
                ))])
                .await
                .expect("set negative timestamp without SQL unary minus");
            assert!(
                DbReplicaSignerRepo::new(engine.client())
                    .trusted_signers(&member.issuer, 10)
                    .await
                    .expect("negative IdP timestamp is excluded")
                    .is_empty(),
                "{table}: {column} = -1"
            );
        }
    }

    #[tokio::test]
    async fn trusted_signers_enforce_issuer_and_approval_expiry_boundaries() {
        let (engine, member, mut signer) = public_trust_fixture().await;
        execute(&engine, "UPDATE idp_replica_signers SET expires_at = 20").await;
        signer.expires_at = Some(20);
        let repo = DbReplicaSignerRepo::new(engine.client());
        for now in [-1, 9, 20, i64::MAX] {
            assert!(
                repo.trusted_signers(&member.issuer, now)
                    .await
                    .expect("date gate")
                    .is_empty()
            );
        }
        for now in [10, 19] {
            assert_eq!(
                repo.trusted_signers(&member.issuer, now)
                    .await
                    .expect("within validity"),
                vec![(member.clone(), signer.clone())]
            );
        }
        for issuer in [
            "",
            "https://other.example",
            "https://installation.example/",
            " https://installation.example",
        ] {
            assert!(
                repo.trusted_signers(issuer, 10)
                    .await
                    .expect("exact canonical issuer")
                    .is_empty()
            );
        }
    }

    #[tokio::test]
    async fn trusted_signers_reject_invalid_public_keys() {
        for (field, value) in [
            ("kid", "other-key"),
            ("use", "enc"),
            ("alg", "ES256"),
            ("crv", "P-256"),
            ("x", "invalid"),
        ] {
            let (engine, member, signer) = public_trust_fixture().await;
            let mut jwk = serde_json::to_value(signer.public_jwk).expect("public JWK JSON");
            jwk[field] = value.into();
            execute(
                &engine,
                &format!(
                    "UPDATE idp_replica_signers SET public_jwk = {}",
                    super::text(&jwk.to_string())
                ),
            )
            .await;
            let result = DbReplicaSignerRepo::new(engine.client())
                .trusted_signers(&member.issuer, 10)
                .await;
            assert!(
                result.is_err() || result.expect("decoded public key").is_empty(),
                "accepted {field}"
            );
        }
        let (engine, member, _) = public_trust_fixture().await;
        execute(
            &engine,
            "UPDATE idp_replica_signers SET public_jwk = 'invalid'",
        )
        .await;
        assert!(
            DbReplicaSignerRepo::new(engine.client())
                .trusted_signers(&member.issuer, 10)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn trusted_signers_reject_ambiguous_idp_bindings() {
        for (table, indexes, columns) in [
            (
                "idp_replica_members",
                "DROP INDEX idp_replica_members_installation_member; DROP INDEX idp_replica_members_installation_endpoint;",
                "installation_id, member_id, endpoint_id, issuer, approved_at, revoked_at",
            ),
            (
                "idp_replica_signers",
                "DROP INDEX idp_replica_signers_key_id; DROP INDEX idp_replica_signers_active_member;",
                "member_id, active_member_id, key_id, issuer, public_jwk, approved_at, revoked_at, expires_at",
            ),
        ] {
            let (engine, member, signer) = public_trust_fixture().await;
            execute(&engine, indexes).await;
            let mut row = engine
                .client()
                .translate_and_execute(
                    &format!("SELECT id, {columns} FROM {table}"),
                    &SqlTranslator,
                )
                .await
                .expect("read duplicate binding fixture")
                .remove(0)
                .rows
                .remove(0);
            row.values[0] = Value::Uuid(Id::now_v7());
            engine
                .client()
                .execute(vec![db::Statement::Query(db::Query::Insert(
                    db::QueryInsert {
                        table: table.into(),
                        row,
                        returning: None,
                    },
                ))])
                .await
                .expect("insert ambiguous IdP binding");
            assert!(
                DbReplicaSignerRepo::new(engine.client())
                    .trusted_signers(&member.issuer, 10)
                    .await
                    .is_err(),
                "{table} ambiguity must deny"
            );
            let error = crate::replica::LocalIdentity::new(engine.client())
                .check_join_identity(
                    &member.issuer,
                    member.member_id,
                    &member.endpoint_id,
                    &signer.public_jwk,
                    Id::now_v7(),
                    10,
                )
                .await
                .expect_err(
                    "identity owner rejects ambiguous enrollment before considering the User",
                );
            assert!(
                error.to_string().contains("ambiguous node trust bindings"),
                "{error}"
            );
        }
    }

    #[tokio::test]
    async fn caller_transaction_controls_enrollment_commit_and_rollback() {
        let engine = Arc::new(db::Database::in_memory());
        replica::up(&engine).await.expect("initialize IdP schema");
        let repo = DbReplicaSignerRepo::new(engine.client());
        let (membership, signer) = records();
        let mut transaction = engine
            .client()
            .transaction()
            .await
            .expect("owner transaction");
        assert!(
            DbReplicaSignerRepo::enroll_in_transaction(&mut transaction, &membership, &signer)
                .await
                .expect("prepare enrollment in caller transaction")
        );
        transaction.rollback().await.expect("caller rollback");
        let results = engine
            .client()
            .translate_and_execute(
                "SELECT id FROM idp_replica_members; SELECT id FROM idp_replica_signers",
                &SqlTranslator,
            )
            .await
            .expect("rolled-back enrollment");
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|result| result.rows.is_empty()));
        let mut transaction = engine
            .client()
            .transaction()
            .await
            .expect("retry owner transaction");
        assert!(
            DbReplicaSignerRepo::enroll_in_transaction(&mut transaction, &membership, &signer)
                .await
                .expect("retry rolled-back enrollment")
        );
        transaction.commit().await.expect("caller commit");
        assert!(
            !repo
                .enroll(&membership, &signer)
                .await
                .expect("committed exact retry")
        );
    }

    #[tokio::test]
    async fn enrollment_is_persistent_idempotent_and_rejects_conflicts() {
        let engine = Arc::new(db::Database::in_memory());
        replica::up(&engine).await.expect("initialize IdP schema");
        let repo = DbReplicaSignerRepo::new(engine.client());
        let (membership, signer) = records();

        assert!(
            repo.enroll(&membership, &signer)
                .await
                .expect("enroll approved signer")
        );
        let mut retried_membership = membership.clone();
        retried_membership.approved_at = 11;
        let mut retried_signer = signer.clone();
        retried_signer.approved_at = Some(11);
        assert!(
            !repo
                .enroll(&retried_membership, &retried_signer)
                .await
                .expect("accept matching enrollment retry with a new server timestamp")
        );
        assert_eq!(
            engine
                .client()
                .translate_and_execute("SELECT id FROM idp_replica_members", &SqlTranslator,)
                .await
                .expect("read membership rows")[0]
                .rows
                .len(),
            1
        );
        assert_eq!(
            engine
                .client()
                .translate_and_execute("SELECT id FROM idp_replica_signers", &SqlTranslator,)
                .await
                .expect("read signer rows")[0]
                .rows
                .len(),
            1
        );

        let mut conflicting_endpoint = membership.clone();
        conflicting_endpoint.endpoint_id = "other-endpoint".to_owned();
        assert!(repo.enroll(&conflicting_endpoint, &signer).await.is_err());
        let mut conflicting_installation = membership.clone();
        conflicting_installation.installation_id = "https://other-installation.example".to_owned();
        assert!(
            repo.enroll(&conflicting_installation, &signer)
                .await
                .is_err()
        );

        let mut replacement = signer.clone();
        replacement.key_id = Id::from_u128(3);
        replacement.public_jwk.kid = replacement.key_id.to_string();
        replacement.approved_at = Some(20);
        assert!(
            repo.rotate(membership.member_id, &membership.issuer, &replacement)
                .await
                .expect("rotate signer")
        );
        assert_eq!(
            repo.trusted_signers(&membership.issuer, 20)
                .await
                .expect("only the replacement signer is active"),
            vec![(membership.clone(), replacement.clone())]
        );
        assert!(
            !repo
                .rotate(membership.member_id, &membership.issuer, &replacement)
                .await
                .expect("accept matching rotation retry")
        );
        let signer_rows = engine
            .client()
            .translate_and_execute(
                "SELECT key_id, revoked_at FROM idp_replica_signers ORDER BY key_id",
                &SqlTranslator,
            )
            .await
            .expect("read rotated signer trust")[0]
            .rows
            .clone();
        assert_eq!(signer_rows.len(), 2);
        assert!(matches!(
            signer_rows[0].values.get(1),
            Some(Value::Integer(20))
        ));
        assert!(matches!(signer_rows[1].values.get(1), Some(Value::Null)));
        assert!(
            repo.revoke(membership.member_id, &membership.issuer, 30)
                .await
                .expect("revoke member and signer")
        );
        assert!(
            repo.trusted_signers(&membership.issuer, 30)
                .await
                .expect("revoked IdP trust is excluded")
                .is_empty()
        );
        assert!(
            !repo
                .revoke(membership.member_id, &membership.issuer, 31)
                .await
                .expect("accept repeated revocation")
        );
        let member_state = engine
            .client()
            .translate_and_execute("SELECT revoked_at FROM idp_replica_members", &SqlTranslator)
            .await
            .expect("read revoked membership")[0]
            .rows
            .clone();
        assert!(matches!(
            member_state[0].values.first(),
            Some(Value::Integer(30))
        ));
    }
}
