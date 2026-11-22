use chrono::Utc;
use of_model::id::Id;
use of_model::idp::contract::permission_evaluation::{INSTALLATION_POLICY_ID, IdentityAction};
use of_model::management_error::{ManagementError, ManagementResult};
use ofdb_sql::{Client, QueryParams, QueryResult, Transaction, Value};

use super::insert::{db_error, policy_insert, returned_id};

const ADMINISTRATOR: &str = "installation-administrator";

/// Provision the original administrator on the Installation's identity owner.
/// Exact setup retries do not change policy, including after a grant or policy is deleted.
pub async fn provision_initial_administrator(
    client: &Client,
    user_id: Id,
    actions: &[IdentityAction],
) -> ManagementResult<()> {
    let mut names = Vec::with_capacity(actions.len());
    for action in actions {
        let name = action.permission();
        if action.application_scoped() || name == "*" || names.contains(&name) {
            return Err(ManagementError::InvalidInput(
                "administrator permissions must be unique explicit installation actions".into(),
            ));
        }
        names.push(name);
    }
    if user_id.is_nil()
        || !names.contains(&IdentityAction::DevicePairingRead.permission())
        || !names.contains(&IdentityAction::DevicePairingUpdate.permission())
    {
        return Err(ManagementError::InvalidInput(
            "administrator requires a valid User and both device-pairing permissions".into(),
        ));
    }
    names.sort_unstable();
    let mut tx = client.transaction().await.map_err(db_error)?;
    match provision(&mut tx, user_id, &names).await {
        Ok(()) => tx.commit().await.map_err(db_error),
        Err(error) => {
            tx.rollback().await.map_err(db_error)?;
            Err(error)
        }
    }
}

async fn provision(tx: &mut Transaction, user: Id, names: &[&str]) -> ManagementResult<()> {
    let key = Value::Text(ADMINISTRATOR.into());
    let user = Value::Uuid(user);
    let inventory = Value::Text(names.join("\n"));
    // The receipt commits with the grants and prevents setup from reissuing deleted policy.
    let receipt = execute(
        tx,
        "SELECT user_id, actions FROM administrator_provisioning WHERE name = $1",
        QueryParams::Positional(vec![key.clone()]),
    )
    .await?;
    if let Some(row) = receipt.first().and_then(|result| result.rows.first()) {
        return if row.values.as_slice() == [user, inventory] {
            Ok(())
        } else {
            Err(ManagementError::InvalidInput(
                "initial administrator setup already used different grants".into(),
            ))
        };
    }
    let now = Utc::now().timestamp();
    let (sql, params) = policy_insert("roles", INSTALLATION_POLICY_ID, ADMINISTRATOR, None, now);
    let role = returned_id(&execute(tx, &sql, params).await?)?;
    for name in names {
        let (sql, params) = policy_insert("permissions", INSTALLATION_POLICY_ID, name, None, now);
        let permission = returned_id(&execute(tx, &sql, params).await?)?;
        execute(tx,
            "INSERT INTO role_permissions (role_id, permission_id, created_at, updated_at) VALUES ($1, $2, $3, $3)",
            QueryParams::Positional(vec![Value::Uuid(role), Value::Uuid(permission), Value::Integer(now)]),
        ).await?;
    }
    execute(tx,
        "INSERT INTO application_user_roles (user_id, application_id, role_id, created_at, updated_at) VALUES ($1, $2, $3, $4, $4)",
        QueryParams::Positional(vec![user.clone(), Value::Uuid(INSTALLATION_POLICY_ID), Value::Uuid(role), Value::Integer(now)]),
    ).await?;
    execute(
        tx,
        "INSERT INTO administrator_provisioning (name, user_id, actions) VALUES ($1, $2, $3)",
        QueryParams::Positional(vec![key, user, inventory]),
    )
    .await?;
    Ok(())
}

async fn execute(
    tx: &mut Transaction,
    sql: &str,
    params: QueryParams,
) -> ManagementResult<Vec<QueryResult>> {
    // Keep the nested SQL transaction future off the setup worker's stack.
    Box::pin(tx.execute_sql(sql, Some(&params)))
        .await
        .map_err(db_error)
}
