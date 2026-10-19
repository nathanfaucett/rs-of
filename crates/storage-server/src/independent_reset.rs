use std::{io, path::Path};

use api::IndependentReset;
use idp_model::model::Id;
use idp_service::setup_credentials::delete_setup_secrets;
use serde::Deserialize;

use crate::AppConfig;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct References {
    version: u8,
    clients: [Id; 2],
    attempted: [bool; 2],
    endpoint: Option<String>,
    domain_attempted: bool,
    complete: bool,
}

pub async fn reset_independent(config: &AppConfig, remove_os_trust: bool) -> io::Result<()> {
    let mut reset =
        IndependentReset::open("storage", Path::new(&config.data_dir), remove_os_trust)?;
    if reset.secrets_pending() {
        if let Some(worker) = reset.worker() {
            let references: References = serde_json::from_value(worker.clone())?;
            if references.version != 1
                || references.clients.iter().any(Id::is_nil)
                || references.clients[0] == references.clients[1]
                || references
                    .endpoint
                    .as_ref()
                    .is_some_and(|endpoint| endpoint.parse::<iroh::EndpointId>().is_err())
                || (references.complete
                    && (!references.domain_attempted
                        || references.endpoint.is_none()
                        || references.attempted != [true; 2]))
                || (!references.domain_attempted && Path::new(&config.data_dir).try_exists()?)
            {
                return Err(io::Error::other("invalid retained Storage reset inventory"));
            }
            let slots = references
                .clients
                .iter()
                .zip(references.attempted)
                .map(|(id, attempted)| (id.to_string(), attempted))
                .collect::<Vec<_>>();
            delete_setup_secrets(
                &format!("independent-setup:{}", reset.directory().display()),
                &slots,
            )?;
        }
        reset.secrets_removed()?;
    }
    reset.finish()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use api::IndependentSetupHost;
    use idp_model::model::Id;

    use super::reset_independent;
    use crate::AppConfig;

    #[tokio::test]
    async fn reset_cannot_remove_a_running_setup_only_storage_host() {
        let domain = std::env::temp_dir().join(format!("storage-reset-{}", Id::now_v7()));
        let config = AppConfig {
            data_dir: domain.to_string_lossy().into_owned(),
            ..Default::default()
        };
        let host = IndependentSetupHost::prepare(
            "storage",
            &domain,
            "127.0.0.1:31992".parse().expect("address"),
            "https://localhost:5173",
            "/storage",
            false,
        )
        .expect("setup")
        .expect("host");
        assert!(reset_independent(&config, true).await.is_err());
        assert!(
            domain
                .with_extension("setup-host")
                .join("host.json")
                .exists()
        );
        drop(host);
        fs::remove_dir_all(domain.with_extension("setup-host")).expect("isolated cleanup");
    }
}
