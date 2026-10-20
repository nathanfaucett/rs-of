use std::{fs, io, path::Path, sync::Arc};

use api::IndependentReset;
use db::Database;
use idp_model::{contract::ClientRegistration, model::Id};
use idp_service::setup_credentials::delete_setup_secrets;
use management_service::independent_join::{IndependentJoinApproval, IndependentJoinRequest};
use serde::Deserialize;

use crate::{AppConfig, OwnerProvisioner};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct References {
    version: u8,
    user: Id,
    credential: Option<Id>,
    login: Id,
    member: Id,
    key: Id,
    client: Id,
    secret_attempted: bool,
    domain_attempted: bool,
    inventory_attempted: bool,
    endpoint: Option<String>,
    registrations: Vec<ClientRegistration>,
    request: Option<IndependentJoinRequest>,
    approval: Option<IndependentJoinApproval>,
    #[serde(default)]
    imported: bool,
    complete: bool,
}

pub async fn reset_independent(config: &AppConfig, remove_os_trust: bool) -> io::Result<()> {
    let mut reset = IndependentReset::open("idp", Path::new(&config.data_dir), remove_os_trust)?;
    if reset.secrets_pending() {
        if let Some(worker) = reset.worker() {
            let references: References = serde_json::from_value(worker.clone())?;
            let intent = reset
                .intent()
                .ok_or_else(|| io::Error::other("worker intent missing"))?;
            let mut ids = vec![
                references.user,
                references.login,
                references.member,
                references.key,
                references.client,
            ];
            ids.extend(references.credential);
            let supported = match (references.version, intent.purpose.as_str()) {
                (1, "create" | "join") | (2, "create") => references.credential.is_some(),
                (2, "join") => worker.get("credential").is_none(),
                _ => false,
            };
            if !supported
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
                || (intent.purpose == "create"
                    && (references.credential.is_none()
                        || references.request.is_some()
                        || references.approval.is_some()
                        || references.imported))
                || (references.version == 2
                    && intent.purpose == "join"
                    && references.credential.is_some())
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
            if let Some(request) = &references.request {
                request.validate()?;
                if request.operation != *intent
                    || request.member_id != references.member
                    || request.public_signer.kid != references.key.to_string()
                    || Some(request.endpoint_id.as_str()) != references.endpoint.as_deref()
                {
                    return Err(io::Error::other("IdP Join reset request changed"));
                }
            }
            if let Some(approval) = &references.approval {
                approval.verify()?;
                if references
                    .request
                    .as_ref()
                    .map(serde_json::to_value)
                    .transpose()?
                    != Some(serde_json::to_value(&approval.request)?)
                    || (references.imported && references.user != approval.owner_subject)
                {
                    return Err(io::Error::other("IdP Join reset approval changed"));
                }
            }
            if intent.purpose == "join"
                && (references.imported || references.complete)
                && (references.request.is_none()
                    || references.approval.is_none()
                    || (references.complete && !references.imported))
            {
                return Err(io::Error::other("completed IdP Join lacks exact approval"));
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
                if clients != expected || (intent.purpose == "create" && owner != references.user) {
                    return Err(io::Error::other(
                        "IdP protected inventory disagrees with worker references",
                    ));
                }
                let database = Path::new(&config.data_dir).join("idp.redb");
                if !fs::symlink_metadata(&database)?.file_type().is_file() {
                    return Err(io::Error::other(
                        "attempted IdP database missing or invalid",
                    ));
                }
                let mut local = config.clone();
                local.oauth2.issuer = intent.issuer.clone();
                // Reuse the owner deletion contract without schema initialization, runtime
                // construction, peer access, secret loading, or key generation.
                let owner_api = OwnerProvisioner::from_database(
                    &local,
                    Arc::new(Database::open(&database).map_err(io::Error::other)?),
                )?;
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
    reset.finish()
}

#[cfg(test)]
mod tests {
    use api::IndependentSetupHost;
    use idp_model::model::Id;

    #[tokio::test]
    async fn reset_cannot_remove_a_running_setup_only_idp_host() {
        let domain = std::env::temp_dir().join(format!("idp-reset-{}", Id::now_v7()));
        let config = crate::AppConfig {
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
        assert!(super::reset_independent(&config, true).await.is_err());
        drop(host);
        std::fs::remove_dir_all(domain.with_extension("setup-host")).expect("isolated cleanup");
    }
}
