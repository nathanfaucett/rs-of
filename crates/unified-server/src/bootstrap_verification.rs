use std::{io, time::Duration};

use crate::{LocalTls, UnifiedRuntime};

impl UnifiedRuntime {
    /// Integration qualification of the HTTPS router and owner clients; closes all owners.
    pub async fn verify_https_bootstrap(self, tls: &LocalTls) -> io::Result<()> {
        self.verify_bootstrap(tls, true).await
    }

    pub(crate) async fn verify_local_bootstrap(self, tls: Option<&LocalTls>) -> io::Result<()> {
        let readiness = match self.node_public_ready().await {
            Ok(true) => Ok(()),
            Ok(false) => Err(failed("local Node public state is not ready")),
            Err(error) => Err(error),
        };
        match (readiness, tls) {
            (Ok(()), Some(tls)) => self.verify_bootstrap(tls, false).await,
            (result, _) => result.and(self.shutdown().await),
        }
    }

    async fn verify_bootstrap(mut self, tls: &LocalTls, qualification: bool) -> io::Result<()> {
        let expected = match LocalTls::api_base(self.local_addr()) {
            Ok(base) => base,
            Err(error) => {
                let _ = self.shutdown().await;
                return Err(error);
            }
        };
        if self.api_base != expected {
            self.shutdown().await?;
            return Err(failed(
                "bootstrap runtime must be configured for its exact HTTPS listener",
            ));
        }
        let listener = match self.take_listener() {
            Ok(listener) => listener,
            Err(error) => {
                let _ = self.shutdown().await;
                return Err(error);
            }
        };
        let cancellation = self.cancellation_token();
        let shutdown = cancellation.clone();
        let router = self.router();
        let listener = tls.listener(listener);
        let mut server = tokio::spawn(async move {
            axum::serve(listener, router.into_make_service())
                .with_graceful_shutdown(shutdown.cancelled_owned())
                .await
        });
        let result = if qualification {
            self.probe_bootstrap().await
        } else {
            async {
                crate::bootstrap_verification::check_owner_health(
                    &format!("{}/idp", self.api_base),
                    "idp",
                    &LocalTls::verification_client()?,
                )
                .await?;
                Ok(())
            }
            .await
        };
        cancellation.cancel();
        let owner_result = self.shutdown().await;
        let listener_result = match tokio::time::timeout(Duration::from_secs(10), &mut server).await
        {
            Ok(result) => result.map_err(io::Error::other).and_then(|result| result),
            Err(_) => {
                server.abort();
                let _ = server.await;
                Err(failed(
                    "production HTTPS listener did not shut down gracefully",
                ))
            }
        };
        result.and(owner_result).and(listener_result)
    }

    pub(crate) async fn probe_bootstrap(&mut self) -> io::Result<()> {
        let http = LocalTls::verification_client()?;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            if crate::bootstrap_verification::check_owner_health(
                &format!("{}/idp", self.api_base),
                "idp",
                &http,
            )
            .await
            .is_ok()
            {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(failed(
                    "production HTTPS listener did not become trusted and healthy",
                ));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        for context in ["idp", "management", "storage"] {
            crate::bootstrap_verification::check_owner_health(
                &format!("{}/{context}", self.api_base),
                context,
                &http,
            )
            .await?;
        }
        if !self.node_public_ready().await? {
            return Err(failed("local Node public state is not ready"));
        }
        self.start_background_tasks().await?;
        let endpoint = self.endpoint_id().to_string();
        let identity = self
            .management_control_plane
            .get_storage_endpoint_identity(&endpoint)
            .await
            .map_err(|_| failed("local Management to IdP lookup failed"))?;
        if identity.endpoint_id != endpoint {
            return Err(failed("Management resolved a different endpoint"));
        }
        Ok(())
    }
}

fn failed(message: &str) -> io::Error {
    io::Error::other(message)
}

pub(crate) async fn check_owner_health(
    base: &str,
    owner: &str,
    client: &reqwest::Client,
) -> io::Result<()> {
    match owner {
        "idp" => {
            let configuration = of_idp_client::apis::configuration::Configuration {
                base_path: base.to_owned(),
                client: client.clone(),
                ..Default::default()
            };
            of_idp_client::apis::default_api::health(&configuration)
                .await
                .map(|_| ())
                .map_err(io::Error::other)
        }
        "management" => {
            let configuration = of_management_client::apis::configuration::Configuration {
                base_path: base.to_owned(),
                client: client.clone(),
                ..Default::default()
            };
            of_management_client::apis::default_api::health(&configuration)
                .await
                .map(|_| ())
                .map_err(io::Error::other)
        }
        "storage" => {
            let configuration = of_storage_client::apis::configuration::Configuration {
                base_path: base.to_owned(),
                client: client.clone(),
                ..Default::default()
            };
            of_storage_client::apis::default_api::health(&configuration)
                .await
                .map(|_| ())
                .map_err(io::Error::other)
        }
        _ => Err(io::Error::other("unknown owner health contract")),
    }
}
