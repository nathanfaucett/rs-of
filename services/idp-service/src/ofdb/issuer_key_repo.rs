use of_model::id::Id;
use of_model::idp::contract::jwk_public::JwkPublic;
use ofdb_sql::{Client, Row, SqlTranslator, Transaction, Value};

use crate::repo::{RepoError, RepoResult};

pub struct DbIssuerKeyRepo {
    engine: Client,
}

impl DbIssuerKeyRepo {
    pub const fn new(engine: Client) -> Self {
        Self { engine }
    }

    pub async fn keys(&self, issuer: &str, now: i64) -> RepoResult<Vec<(JwkPublic, bool)>> {
        let mut transaction = self.engine.transaction().await.map_err(RepoError::other)?;
        let result = Self::read(&mut transaction, issuer, now).await;
        transaction.rollback().await.map_err(RepoError::other)?;
        result
    }

    async fn read(
        transaction: &mut Transaction,
        issuer: &str,
        now: i64,
    ) -> RepoResult<Vec<(JwkPublic, bool)>> {
        let mut results = transaction.translate_and_execute(&format!(
            "SELECT id, key_id, public_jwk, active_issuer FROM issuer_keys WHERE issuer = {} AND (expires_at IS NULL OR expires_at > {now})", text(issuer)), &SqlTranslator).await.map_err(RepoError::other)?;
        let rows = results.pop().ok_or_else(invalid)?.rows;
        for row in &rows {
            let Some(Value::Uuid(id)) = row.values.first() else {
                return Err(invalid());
            };
            if !transaction
                .row_conflicts("issuer_keys", &Row::new(vec![Value::Uuid(*id)]))
                .await
                .map_err(RepoError::other)?
                .is_empty()
            {
                return Err(invalid());
            }
        }
        let keys = rows
            .into_iter()
            .map(|row| {
                let [_, Value::Uuid(id), Value::Text(encoded), slot] = row.values.as_slice() else {
                    return Err(invalid());
                };
                let jwk: JwkPublic = serde_json::from_str(encoded).map_err(RepoError::other)?;
                if jwk.kid != id.to_string() || crate::oauth2::verifing_key_from_jwt(&jwk).is_err()
                {
                    return Err(invalid());
                }
                let active = match slot {
                    Value::Null => false,
                    Value::Text(value) if value == issuer => true,
                    _ => return Err(invalid()),
                };
                Ok((jwk, active))
            })
            .collect::<RepoResult<Vec<_>>>()?;
        if keys.iter().filter(|(_, active)| *active).count() > 1 {
            return Err(invalid());
        }
        Ok(keys)
    }

    pub async fn install_initial(&self, issuer: &str, key: &JwkPublic) -> RepoResult<()> {
        self.publish(issuer, key, None).await
    }

    pub async fn rotate(
        &self,
        issuer: &str,
        key: &JwkPublic,
        previous: &str,
        expires: i64,
    ) -> RepoResult<()> {
        self.publish(issuer, key, Some((previous, expires))).await
    }

    async fn publish(
        &self,
        issuer: &str,
        key: &JwkPublic,
        replacement: Option<(&str, i64)>,
    ) -> RepoResult<()> {
        let id: Id = key.kid.parse().map_err(RepoError::other)?;
        if issuer.is_empty()
            || id.is_nil()
            || key.kid != id.to_string()
            || crate::oauth2::verifing_key_from_jwt(key).is_err()
        {
            return Err(invalid());
        }
        let mut transaction = self.engine.transaction().await.map_err(RepoError::other)?;
        let result = async {
            let keys = Self::read(&mut transaction, issuer, 0).await?;
            match replacement {
                None if keys.is_empty() => (),
                None if keys.iter().any(|(current, _)| current == key) => return Ok(()),
                Some((previous, expires)) if keys.iter().any(|(current, active)| *active && current.kid == previous) => {
                    transaction.translate_and_execute(&format!("UPDATE issuer_keys SET active_issuer = NULL, expires_at = {expires} WHERE active_issuer = {}", text(issuer)), &SqlTranslator).await.map_err(RepoError::other)?;
                }
                _ => return Err(invalid()),
            }
            let encoded = serde_json::to_string(key).map_err(RepoError::other)?;
            transaction.translate_and_execute(&format!(
                "INSERT INTO issuer_keys (issuer, active_issuer, key_id, public_jwk, expires_at) VALUES ({0}, {0}, CAST('{id}' AS UUID), {1}, NULL)", text(issuer), text(&encoded)), &SqlTranslator).await.map_err(RepoError::other)?;
            Ok(())
        }.await;
        match result {
            Ok(()) => transaction.commit().await.map_err(RepoError::other),
            Err(error) => {
                transaction.rollback().await.map_err(RepoError::other)?;
                Err(error)
            }
        }
    }

    pub async fn withdraw(&self, issuer: &str, key: Id, now: i64) -> RepoResult<()> {
        self.engine.translate_and_execute(&format!("UPDATE issuer_keys SET active_issuer = NULL, expires_at = {now} WHERE issuer = {} AND key_id = CAST('{key}' AS UUID)", text(issuer)), &SqlTranslator).await.map_err(RepoError::other)?;
        Ok(())
    }
}

fn text(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn invalid() -> RepoError {
    RepoError::InvalidInput("invalid or conflicting issuer key state".into())
}
