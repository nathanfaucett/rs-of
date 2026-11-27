use std::{io, path::Path, sync::Arc};

use axum::{Router, http::HeaderValue};
use db::EmbeddedDatabase;
use device_service::{
    DeviceIdentity, DeviceRepo, HostedControlPlane,
    ofdb::{DbDeviceRepo, DbSelectionPolicyRepo},
};
use idp_service::setup_credentials::SetupRelationship;
use of_model::id::Id;
use serde::{Deserialize, Serialize};
use server_common::{ServerConfig, random_capability};
use tokio_util::sync::CancellationToken;
use tower_http::cors::{Any, CorsLayer};

use crate::{
    compose_router,
    node_idp::{NodeIdp, STORAGE_SYNC_AUDIENCE, build_node_idp},
};

#[cfg(test)]
#[path = "node_runtime_tests.rs"]
pub mod tests;

#[derive(Deserialize, Serialize)]
pub struct CreateRecord {
    pub administrator: String,
    pub nonce: String,
    user: Id,
    pub login: Id,
    member: Id,
    key: Id,
    endpoint: [u8; 32],
    clients: [(Id, String); 4],
    pub device: Option<Id>,
}

impl CreateRecord {
    pub fn new(administrator: String) -> io::Result<Self> {
        let client = || Ok::<_, io::Error>((Id::now_v7(), random_capability()?));
        Ok(Self {
            administrator,
            nonce: random_capability()?[..32].into(),
            user: Id::now_v7(),
            login: Id::now_v7(),
            member: Id::now_v7(),
            key: Id::now_v7(),
            endpoint: iroh::SecretKey::generate().to_bytes(),
            clients: [client()?, client()?, client()?, client()?],
            device: None,
        })
    }
}

pub struct NodeRuntime {
    pub root: std::path::PathBuf,
    pub base: String,
    pub ui: String,
    pub pm: String,
    pub client: reqwest::Client,
    pub database: EmbeddedDatabase,
}

impl NodeRuntime {
    pub async fn build(
        &self,
        record: &mut CreateRecord,
        password: Option<&str>,
        cancelled: &CancellationToken,
        shutdown: &CancellationToken,
    ) -> io::Result<Router> {
        let idp_base = format!("{}/idp", self.base);
        let device_base = format!("{}/device", self.base);
        let storage_base = format!("{}/storage", self.base);
        let engine = self.database.client();
        let secret = iroh::SecretKey::from_bytes(&record.endpoint);
        let endpoint = secret.public();
        let evaluator = 0;
        let control = 1;
        let storage_idp = 2;
        let storage_device = 3;
        let registrations = [
            SetupRelationship::IdpManagement,
            SetupRelationship::ManagementIdp,
            SetupRelationship::StorageIdp,
            SetupRelationship::StorageManagement,
        ]
        .into_iter()
        .zip(&record.clients)
        .map(|(relationship, (id, secret))| {
            let mut registration =
                relationship.registration(*id, &idp_base, &device_base, &storage_base);
            if relationship == SetupRelationship::StorageIdp {
                registration
                    .allowed_audiences
                    .push(STORAGE_SYNC_AUDIENCE.into());
            }
            registration.client_secret = Some(secret.clone());
            registration
        })
        .collect::<Vec<_>>();
        let mut config = idp_server::AppConfig {
            server: ServerConfig {
                prefix: Some("/idp".into()),
                ..Default::default()
            },
            api_public_uri: idp_base.clone(),
            ui_public_uri: self.ui.clone(),
            service_audience: Some(idp_base.clone()),
            ..Default::default()
        };
        config.oauth2.issuer.clone_from(&idp_base);
        let (idp, login_application) = build_node_idp(
            &config,
            &self.database,
            NodeIdp {
                user_id: record.user,
                username: &record.administrator,
                login_id: record.login,
                member_id: record.member,
                key_id: record.key,
                endpoint_id: endpoint,
                password,
                device_uri: &device_base,
                storage_uri: &storage_base,
                storage_introspection_client: record.clients[storage_idp].0,
                clients: &registrations,
            },
        )
        .await?;
        if cancelled.is_cancelled() {
            return Err(io::ErrorKind::Interrupted.into());
        }
        if password.is_some() {
            management_service::ofdb::provision_initial_operator(
                &engine,
                record.user,
                login_application,
            )
            .await
            .map_err(io::Error::other)?;
            record.device = Some(
                device_server::provision_initial_device(
                    engine.clone(),
                    record.device,
                    record.user,
                    &endpoint.to_string(),
                    "My Node",
                )
                .await?,
            );
        }
        let device = DbDeviceRepo::new(engine.clone())
            .find_approved_by_public_key(&endpoint.to_string())
            .await
            .map_err(io::Error::other)?
            .filter(|device| Some(device.id) == record.device)
            .ok_or_else(|| io::Error::other("approved Node Device is missing"))?;
        let policies = DbSelectionPolicyRepo::new(engine.clone());
        if policies
            .local_cleanup_pending(device.id)
            .await
            .map_err(io::Error::other)?
            || !policies
                .get(device.id, &device.owner_subject, Id::nil())
                .await
                .map_err(io::Error::other)?
                .is_some_and(|policy| policy.admin_allowed)
        {
            return Err(io::Error::other("Node Device policy is unavailable"));
        }

        let network = iroh::Endpoint::builder(iroh::endpoint::presets::Minimal)
            .secret_key(secret.clone())
            .bind()
            .await
            .map_err(io::Error::other)?;
        let plane = Arc::new(
            HostedControlPlane::new_with_services(&idp_base, &storage_base, &idp_base)
                .and_then(|plane| {
                    plane.with_idp_service_client(
                        record.clients[control].0.to_string(),
                        &record.clients[control].1,
                        &idp_base,
                    )
                })
                .and_then(|plane| {
                    plane.with_permission_evaluator(&record.clients[evaluator].0.to_string())
                })
                .map_err(io::Error::other)?
                .with_http_client(self.client.clone())
                .with_device_identity(Arc::new(DeviceIdentity::new(network, secret)?))
                .with_local_devices(Arc::new(DbDeviceRepo::new(engine.clone()))),
        );
        let device = device_server::build_router(
            engine.clone(),
            engine.clone(),
            &device_base,
            "/device",
            &storage_base,
            plane.clone(),
        )
        .await
        .map_err(io::Error::other)?;
        let management = management_server::build_router(
            engine,
            &format!("{}/management", self.base),
            "/management",
            plane,
        )
        .await
        .map_err(io::Error::other)?;
        let storage = storage_server::RouterState::new(&storage_base, endpoint.to_string())
            .with_idp_client(
                storage_server::IdpClient::new(
                    &idp_base,
                    &record.clients[storage_idp].0.to_string(),
                    &record.clients[storage_idp].1,
                    &idp_base,
                    &idp_base,
                )
                .map_err(io::Error::other)?
                .with_http_client(self.client.clone()),
            )
            .with_device_client(
                storage_server::DeviceClient::new(
                    &device_base,
                    &idp_base,
                    &record.clients[storage_device].0.to_string(),
                    &record.clients[storage_device].1,
                    &idp_base,
                    &storage_base,
                )
                .map_err(io::Error::other)?
                .with_http_client(self.client.clone()),
            );
        let storage = storage_server::build_shared_http_runtime(
            storage,
            Path::new(&self.root),
            "/storage",
            endpoint,
            &self.database,
            shutdown.child_token(),
        )?;
        let ui = server_common::setup_origin(&self.ui)?;
        let pm = server_common::setup_origin(&self.pm)?;
        let cors = |origins: Vec<HeaderValue>| {
            CorsLayer::new()
                .allow_origin(origins)
                .allow_methods(Any)
                .allow_headers(Any)
                .allow_private_network(true)
        };
        Ok(compose_router(
            idp.layer(cors(vec![ui.clone(), pm.clone()])),
            management.layer(cors(vec![ui.clone()])),
            device.layer(cors(vec![ui.clone()])),
            storage.router().layer(cors(vec![ui, pm])),
        ))
    }
}

#[cfg(test)]
#[path = "node_publish_tests.rs"]
mod publish_tests;
