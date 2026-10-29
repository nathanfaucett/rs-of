use std::{io, path::Path};

use api::{IndependentPeer, IndependentReset};
use device_service::independent_join::IndependentJoinApproval;
use idp_model::model::Id;
use idp_service::setup_credentials::delete_setup_secrets;
use management_service::INITIAL_ADMINISTRATOR_ACTIONS;
use serde::Deserialize;

use crate::AppConfig;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateSeed {
    role: Id,
    permissions: Vec<Id>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct References {
    version: u8,
    client: Id,
    device: Id,
    create: Option<CreateSeed>,

    user: Option<Id>,
    evaluator: Option<Id>,
    secret_attempted: bool,
    endpoint_attempted: bool,
    endpoint: Option<String>,
    domain_attempted: bool,
    source: Option<IndependentPeer>,
    approval: Option<IndependentJoinApproval>,
    #[serde(default)]
    imported: bool,
    complete: bool,
}

pub async fn reset_independent(config: &AppConfig, remove_os_trust: bool) -> io::Result<()> {
    if let Some(remote) = &config.device_database {
        remote.validate()?;
        let domain = Path::new(&config.data_dir);
        let mut retained = false;
        for path in [
            domain.to_path_buf(),
            domain.with_extension("setup-host"),
            domain.with_extension("setup-reset"),
            domain.with_extension("setup-reset.json"),
        ] {
            retained |= path.is_symlink() || path.try_exists()?;
        }
        if !retained {
            return Ok(());
        }
        if domain.is_symlink()
            || (domain.try_exists()? && std::fs::read_dir(domain)?.next().is_some())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "remote Device consumer root must be empty; preserve unexpected owner data",
            ));
        }
    }
    let mut reset = IndependentReset::open("device", Path::new(&config.data_dir), remove_os_trust)?;
    if reset.secrets_pending() {
        if let Some(worker) = reset.worker() {
            let references: References = serde_json::from_value(worker.clone())?;
            let intent = reset
                .intent()
                .ok_or_else(|| io::Error::other("worker intent missing"))?;
            let mut ids = vec![references.client, references.device];
            if references.version != 2 {
                return Err(io::Error::other("unsupported Device reset record"));
            }
            let seed = references
                .create
                .as_ref()
                .map(|seed| (seed.role, seed.permissions.as_slice()));
            if let Some((role, permissions)) = seed {
                if permissions.len() != INITIAL_ADMINISTRATOR_ACTIONS.len() {
                    return Err(io::Error::other("invalid retained administrator seed"));
                }
                ids.push(role);
                ids.extend(permissions);
            }
            if ids.iter().any(Id::is_nil)
                || ids.iter().enumerate().any(|(i, id)| ids[..i].contains(id))
                || references.user.is_some_and(|id| id.is_nil())
                || references.evaluator.is_some_and(|id| id.is_nil())
                || references
                    .endpoint
                    .as_ref()
                    .is_some_and(|endpoint| endpoint.parse::<iroh::EndpointId>().is_err())
                || (intent.purpose == "create"
                    && (seed.is_none()
                        || references.source.is_some()
                        || references.approval.is_some()
                        || references.imported))
                || (intent.purpose == "join" && worker.get("create").is_some())
                || (references.complete
                    && (!references.domain_attempted
                        || !references.secret_attempted
                        || !references.endpoint_attempted
                        || references.endpoint.is_none()
                        || references.user.is_none()
                        || references.evaluator.is_none()))
                || (!references.domain_attempted && Path::new(&config.data_dir).try_exists()?)
            {
                return Err(io::Error::other(
                    "invalid retained Management reset inventory",
                ));
            }
            if let Some(source) = &references.source {
                source.verify(
                    intent
                        .peer
                        .as_deref()
                        .ok_or_else(|| io::Error::other("Join peer missing"))?,
                    &source.nonce,
                )?;
                if source.issuer != intent.issuer {
                    return Err(io::Error::other("Join issuer changed"));
                }
            }
            if let Some(approval) = &references.approval {
                approval.verify()?;
                if approval.request.operation != *intent
                    || approval.request.device_id != references.device
                    || Some(approval.request.endpoint_id.as_str()) != references.endpoint.as_deref()
                    || Some(approval.owner_subject) != references.user
                    || references
                        .source
                        .as_ref()
                        .map(serde_json::to_value)
                        .transpose()?
                        != Some(serde_json::to_value(&approval.request.source)?)
                {
                    return Err(io::Error::other("Management Join reset binding changed"));
                }
            }
            if intent.purpose == "join"
                && (references.imported || references.complete)
                && (references.source.is_none()
                    || references.approval.is_none()
                    || (references.complete && !references.imported))
            {
                return Err(io::Error::other("completed Join lacks exact approval"));
            }
            delete_setup_secrets(
                &format!("independent-setup:{}", reset.directory().display()),
                &[
                    (references.client.to_string(), references.secret_attempted),
                    ("endpoint".into(), references.endpoint_attempted),
                ],
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
    async fn reset_cannot_remove_a_running_setup_only_management_host() {
        let domain = std::env::temp_dir().join(format!("device-reset-{}", Id::now_v7()));
        let config = crate::AppConfig {
            data_dir: domain.to_string_lossy().into_owned(),
            ..Default::default()
        };
        let host = IndependentSetupHost::prepare(
            "device",
            &domain,
            "127.0.0.1:31993".parse().expect("address"),
            "https://localhost:5173",
            "/device",
            false,
        )
        .expect("prepare")
        .expect("host");
        assert!(super::reset_independent(&config, true).await.is_err());
        drop(host);
        std::fs::remove_dir_all(domain.with_extension("setup-host")).expect("isolated cleanup");
    }
}
