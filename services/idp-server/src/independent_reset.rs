#[cfg(feature = "independent")]
use std::{fs, io, path::Path, sync::Arc};

use idp_service::setup_credentials::delete_setup_secrets;
use of_model::id::Id;
use of_model::idp::contract::client_registration::ClientRegistration;
use ofdb_sql::Database;
use serde::Deserialize;
use setup::independent_reset::IndependentReset;

use crate::{config::app_config::AppConfig, provisioning::OwnerProvisioner};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct References {
    version: u8,
    user: Id,
    login: Id,
    member: Id,
    key: Id,
    client: Id,
    secret_attempted: bool,
    domain_attempted: bool,
    inventory_attempted: bool,
    endpoint: Option<String>,
    registrations: Vec<ClientRegistration>,
    complete: bool,
}

pub async fn reset_independent(
    config: &AppConfig,
    remove_os_trust: bool,
    acknowledge: impl FnOnce() -> io::Result<()>,
) -> io::Result<()> {
    let mut reset = IndependentReset::open("idp", Path::new(&config.data_dir), remove_os_trust)?;
    if reset.secrets_pending() {
        if let Some(worker) = reset.worker() {
            let references: References = serde_json::from_value(worker.clone())?;
            let intent = reset
                .intent()
                .ok_or_else(|| io::Error::other("worker intent missing"))?;
            let ids = vec![
                references.user,
                references.login,
                references.member,
                references.key,
                references.client,
            ];
            if references.version != 2
                || ids.iter().any(Id::is_nil)
                || ids.iter().enumerate().any(|(i, id)| ids[..i].contains(id))
                || references
                    .registrations
                    .iter()
                    .any(|r| r.client_secret.is_some())
                || references
                    .endpoint
                    .as_ref()
                    .is_some_and(|endpoint| endpoint.parse::<iroh::EndpointId>().is_err())
                || (references.complete
                    && (!references.domain_attempted
                        || !references.inventory_attempted
                        || !references.secret_attempted
                        || references.endpoint.is_none()
                        || references.registrations.len() != 3))
                || (!references.domain_attempted && Path::new(&config.data_dir).try_exists()?)
            {
                return Err(io::Error::other("invalid retained IdP reset inventory"));
            }
            let inventory_path = Path::new(&config.data_dir).join("setup-secret-inventory.json");
            if references.inventory_attempted {
                let inventory = reset.read_domain_document("setup-secret-inventory.json")?;
                let owner: Id = serde_json::from_value(inventory["owner"].clone())?;
                let clients: Vec<Id> = serde_json::from_value(inventory["clients"].clone())?;
                let mut expected = vec![references.login, references.client];
                for registration in &references.registrations {
                    expected.push(
                        registration
                            .client_id
                            .as_deref()
                            .ok_or_else(|| io::Error::other("missing exact Client reference"))?
                            .parse::<Id>()
                            .map_err(io::Error::other)?,
                    );
                }
                if clients != expected || owner != references.user {
                    return Err(io::Error::other(
                        "IdP protected inventory disagrees with worker references",
                    ));
                }
                let mut local = config.clone();
                local.oauth2.issuer = intent.issuer.clone();
                let owner_api = if config.identity_database.is_some() {
                    OwnerProvisioner::open(&local).await?
                } else {
                    let database = Path::new(&config.data_dir).join("idp.redb");
                    if !fs::symlink_metadata(&database)?.file_type().is_file() {
                        return Err(io::Error::other(
                            "attempted IdP database missing or invalid",
                        ));
                    }
                    OwnerProvisioner::from_database(
                        &local,
                        Arc::new(Database::open(&database).map_err(io::Error::other)?),
                    )?
                };
                owner_api
                    .delete_local_secrets(references.member, references.key, owner, &clients)
                    .await?;
            } else if inventory_path.try_exists()?
                || Path::new(&config.data_dir)
                    .join("local-client-key-references")
                    .try_exists()?
            {
                return Err(io::Error::other(
                    "unexpected IdP key inventory; preserve evidence",
                ));
            }
            delete_setup_secrets(
                &format!("independent-setup:{}", reset.directory().display()),
                &[(references.client.to_string(), references.secret_attempted)],
            )?;
        }
        reset.secrets_removed()?;
    }
    reset.finish(acknowledge)
}

#[cfg(test)]
mod tests {
    use of_model::id::Id;
    use setup::independent_setup::IndependentSetupHost;

    #[tokio::test]
    async fn reset_cannot_remove_a_running_setup_only_idp_host() {
        let domain = std::env::temp_dir().join(format!("idp-reset-{}", Id::now_v7()));
        let config = crate::config::app_config::AppConfig {
            data_dir: domain.to_string_lossy().into_owned(),
            ..Default::default()
        };
        let host = IndependentSetupHost::prepare(
            "idp",
            &domain,
            "127.0.0.1:31994".parse().expect("address"),
            "https://localhost:5173",
            "/idp",
            false,
        )
        .expect("prepare")
        .expect("host");
        assert!(
            crate::independent_reset::reset_independent(&config, true, || Ok(()))
                .await
                .is_err()
        );
        drop(host);
        std::fs::remove_dir_all(domain.with_extension("setup-host")).expect("isolated cleanup");
    }
}
