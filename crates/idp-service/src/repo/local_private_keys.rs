use std::future::Future;

use db::{Client, QueryParams, Row, Value, decode};
use key::{DerivationPath, DerivedKey};
use of_model::idp::model::Id;

use crate::repo::{RepoError, RepoResult};

pub struct LocalPrivateKeys(Client);

impl LocalPrivateKeys {
    pub fn new(client: Client) -> RepoResult<Self> {
        run(async {
            client.execute_sql("CREATE TABLE IF NOT EXISTS local_private_keys (id UUID PRIMARY KEY, secret TEXT)", None).await.map_err(error)?;
            Ok(())
        })?;
        Ok(Self(client))
    }

    pub fn load(
        &self,
        namespace: &str,
        derivation: &DerivationPath,
    ) -> RepoResult<Option<DerivedKey>> {
        run(async {
            let id = key_id(namespace, derivation);
            let params = QueryParams::Positional(vec![Value::Uuid(id)]);
            let mut transaction = self.0.transaction().await.map_err(error)?;
            if !transaction
                .row_conflicts("local_private_keys", &Row::new(vec![Value::Uuid(id)]))
                .await
                .map_err(error)?
                .is_empty()
            {
                return Err(RepoError::InvalidInput(
                    "conflicted local private key".into(),
                ));
            }
            let rows = transaction
                .execute_sql(
                    "SELECT secret FROM local_private_keys WHERE id = $1",
                    Some(&params),
                )
                .await
                .map_err(error)?;
            let secret: Option<String> = rows[0]
                .rows
                .first()
                .map(|row| decode(&row.values[0], "secret").map_err(error))
                .transpose()?
                .flatten();
            transaction.rollback().await.map_err(error)?;
            secret
                .map(|secret| DerivedKey::from_xprv(secret, derivation.clone()).map_err(Into::into))
                .transpose()
        })
    }

    pub fn store(&self, namespace: &str, key: &DerivedKey) -> RepoResult<()> {
        self.write(
            namespace,
            key.derivation_path(),
            Some(key.to_xprv_string().to_string()),
        )
    }

    pub fn delete(&self, namespace: &str, derivation: &DerivationPath) -> RepoResult<()> {
        self.write(namespace, derivation, None)
    }

    fn write(
        &self,
        namespace: &str,
        derivation: &DerivationPath,
        secret: Option<String>,
    ) -> RepoResult<()> {
        run(async {
            let id = key_id(namespace, derivation);
            let mut transaction = self.0.transaction().await.map_err(error)?;
            let params = QueryParams::Positional(vec![
                Value::Uuid(id),
                secret.map_or(Value::Null, Value::Text),
            ]);
            let id_params = QueryParams::Positional(vec![Value::Uuid(id)]);
            let rows = transaction
                .execute_sql(
                    "SELECT id FROM local_private_keys WHERE id = $1",
                    Some(&id_params),
                )
                .await
                .map_err(error)?;
            let sql = if rows[0].rows.is_empty() {
                "INSERT INTO local_private_keys (id, secret) VALUES ($1, $2)"
            } else {
                "UPDATE local_private_keys SET secret = $2 WHERE id = $1"
            };
            transaction
                .execute_sql(sql, Some(&params))
                .await
                .map_err(error)?;
            transaction.commit().await.map_err(error)
        })
    }
}

fn key_id(namespace: &str, derivation: &DerivationPath) -> Id {
    let namespace = Id::new_v5(&Id::NAMESPACE_URL, namespace.as_bytes());
    Id::new_v5(&namespace, derivation.to_string().as_bytes())
}

fn run<T: Send>(future: impl Future<Output = RepoResult<T>> + Send) -> RepoResult<T> {
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                tokio::runtime::Builder::new_current_thread()
                    .build()
                    .map_err(error)?
                    .block_on(future)
            })
            .join()
            .map_err(|_| RepoError::InvalidInput("local private key worker panicked".into()))?
    })
}

fn error(error: impl std::fmt::Display) -> RepoError {
    RepoError::InvalidInput(error.to_string())
}
