use std::{io, sync::Arc, time::Duration};

use axum::Router;
use db::Database;
use idp_model::model::Id;
use idp_service::{
    oauth2::OAuth2Service,
    replica::{
        DbApplicationRepo, DbClientRepo, DbKeyRepo, DbOAuth2AuthorizationCodeRepo,
        DbOAuth2RefreshTokenRepo, DbOAuth2UserConsentRepo, DbUserRepo,
    },
    repo::{KeyService, PrivateKeyKeyringRepo, UserRepo},
};
use iroh::EndpointId;
use iroh::protocol::{ProtocolHandler, Router as IrohRouter};
use iroh_chain::Server;
use management_service::DeviceEndpointIdentity;
use management_service::{
    DeviceIdentity, DeviceRepo, HostedControlPlane, LocalPermissions, PairingAcceptanceController,
    TimedPairingAcceptanceController, replica::DbDeviceRepo,
};

use crate::{
    AppConfig, BootstrapProtocolHandler, ManagementPermissionClient, RouterState,
    bootstrap::BOOTSTRAP_ALPN, router::openapi_router,
};

pub struct IdpRuntime {
    router: Router,
    bootstrap_protocol: BootstrapProtocolHandler,

    users: DbUserRepo,
    join_state: RouterState,
}

impl IdpRuntime {
    pub async fn authorize_management_bearer(
        &self,
        bearer: &str,
    ) -> Result<idp_model::contract::IntrospectionResponse, String> {
        crate::management_owner::authorize_management_bearer(&self.join_state, bearer).await
    }

    pub async fn approved_management_device(
        &self,
        device_id: Id,
    ) -> Result<DeviceEndpointIdentity, String> {
        crate::management_owner::approved_management_device(&self.join_state, self, device_id).await
    }

    pub async fn authorize_storage_bearer(
        &self,
        bearer: &str,
    ) -> Result<idp_model::contract::IntrospectionResponse, idp_model::contract::ErrorResponse>
    {
        crate::storage_owner::authorize_storage_bearer(&self.join_state, bearer).await
    }

    pub async fn approved_storage_endpoints(&self) -> io::Result<Vec<String>> {
        Ok(self
            .approved_peer_ids()
            .await?
            .into_iter()
            .map(|endpoint| endpoint.to_string())
            .collect())
    }

    pub async fn authorize_node_join_approver(
        &self,
        bearer: &str,
    ) -> io::Result<idp_model::model::Id> {
        self.join_state.authorize_node_join_approver(bearer).await
    }

    pub async fn node_join_accepting(
        &self,
        bearer: &str,
        accepting: Option<bool>,
    ) -> io::Result<bool> {
        self.authorize_node_join_approver(bearer).await?;
        if let Some(accepting) = accepting {
            self.join_state
                .pairing_acceptance
                .set_pairing_accepting(accepting)
                .map_err(io::Error::other)?;
        }
        self.join_state
            .pairing_acceptance
            .pairing_accepting()
            .map_err(io::Error::other)
    }

    pub async fn approve_node_join(
        &self,
        offer: &iroh_chain::PairingOffer,
        bearer: &str,
    ) -> io::Result<crate::NodeJoinApproval> {
        self.join_state.approve_node_join(offer, bearer).await
    }

    pub async fn enroll_node_join_offer(
        &self,
        offer: &iroh_chain::PairingOffer,
        bearer: &str,
    ) -> io::Result<crate::NodeJoinApproval> {
        self.join_state.enroll_node_join_offer(offer, bearer).await
    }

    pub async fn verify_node_join_enrollment(
        &self,
        approval: &crate::NodeJoinApproval,
    ) -> io::Result<()> {
        self.join_state.verify_node_join_enrollment(approval).await
    }

    pub async fn reply_node_join(
        &self,
        offer: &iroh_chain::PairingOffer,
        bearer: &str,
        approval: &crate::NodeJoinApproval,
    ) -> io::Result<()> {
        self.join_state
            .reply_node_join(offer, bearer, approval)
            .await
    }

    pub fn router(&self) -> Router {
        self.router.clone()
    }

    pub fn bootstrap_protocol(&self) -> BootstrapProtocolHandler {
        self.bootstrap_protocol.clone()
    }

    pub fn protocol_router<D>(&self, server: &Server, data_protocol: D) -> IrohRouter
    where
        D: ProtocolHandler,
    {
        server.router_with_protocol(data_protocol, BOOTSTRAP_ALPN, self.bootstrap_protocol())
    }

    pub async fn approved_device_owner(
        &self,
        endpoint: EndpointId,
    ) -> io::Result<DeviceEndpointIdentity> {
        let endpoint_id = endpoint.to_string();
        let device = self
            .join_state
            .devices
            .find_approved_by_public_key(&endpoint_id)
            .await
            .map_err(io::Error::other)?
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "endpoint has no uniquely approved Device",
                )
            })?;
        let owner: Id = device.owner_subject.parse().map_err(io::Error::other)?;
        if owner.is_nil()
            || owner.to_string() != device.owner_subject
            || self
                .users
                .find_user_by_id(owner)
                .await
                .map_err(io::Error::other)?
                .is_none()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Device owner is not locally active",
            ));
        }
        Ok(DeviceEndpointIdentity {
            device_id: device.id,
            owner_subject: device.owner_subject,
            endpoint_id,
        })
    }

    pub async fn approved_peer_ids(&self) -> io::Result<Vec<EndpointId>> {
        let mut endpoints = Vec::new();
        for device in self
            .join_state
            .devices
            .list()
            .await
            .map_err(io::Error::other)?
        {
            if device.state != management_service::DeviceState::Approved
                || device.revoked_at.is_some()
            {
                continue;
            }
            let endpoint = device
                .public_key
                .parse::<EndpointId>()
                .map_err(io::Error::other)?;
            if endpoint.to_string() != device.public_key {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "noncanonical Device endpoint",
                ));
            }
            self.approved_device_owner(endpoint).await?;
            endpoints.push(endpoint);
        }
        Ok(endpoints)
    }
}

/// Composes serving from an already initialized owner database.
pub async fn build_runtime(
    config: &AppConfig,
    engine: Arc<Database>,
    device_identity: Arc<DeviceIdentity>,
    server: Server,
    local_permissions: Option<LocalPermissions>,
) -> io::Result<IdpRuntime> {
    let remote_permissions = if local_permissions.is_some() {
        None
    } else if let Some(remote) = config.management_permissions.as_ref() {
        if config.management_data_dir.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "remote Management evaluation and a local Management data directory are mutually exclusive",
            ));
        }
        Some(ManagementPermissionClient::new(remote)?)
    } else {
        None
    };
    if device_identity.endpoint_id() != server.endpoint().id() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "device identity does not match the injected Iroh server",
        ));
    }

    if config
        .oauth2
        .local_signer
        .as_ref()
        .is_some_and(|binding| binding.endpoint_id != device_identity.endpoint_id().to_string())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "local signer endpoint does not match the injected Node endpoint",
        ));
    }

    let key_service = Arc::new(KeyService::new(
        DbKeyRepo::new(Arc::clone(&engine)),
        PrivateKeyKeyringRepo::new(&config.oauth2.issuer).map_err(io::Error::other)?,
        config.key_namespace.clone(),
    ));
    if let Some(binding) = config.oauth2.local_signer.as_ref() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_secs()
            .try_into()
            .map_err(io::Error::other)?;
        key_service
            .load_node_signer(binding, &config.oauth2.issuer, now)
            .await
            .map_err(io::Error::other)?;
    }
    let oauth2_service = Arc::new(OAuth2Service::new(
        DbApplicationRepo::new(Arc::clone(&engine)),
        DbClientRepo::new(Arc::clone(&engine), Arc::clone(&key_service)),
        DbOAuth2AuthorizationCodeRepo::new(Arc::clone(&engine)),
        DbOAuth2RefreshTokenRepo::new(Arc::clone(&engine)),
        DbUserRepo::new(Arc::clone(&engine), config.password.clone()),
        DbOAuth2UserConsentRepo::new(Arc::clone(&engine)),
        key_service,
        config.oauth2.clone(),
    ));

    let control_plane = config
        .control_plane_uri
        .as_deref()
        .map(HostedControlPlane::new)
        .transpose()
        .map_err(io::Error::other)?
        .map(Arc::new);
    let service_audience = config
        .service_audience
        .as_deref()
        .unwrap_or(&config.api_public_uri);
    if service_audience.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "service_audience must not be empty",
        ));
    }

    let management_engine = match config.management_data_dir.as_deref() {
        Some(directory) => {
            if std::path::Path::new(directory) == std::path::Path::new(&config.data_dir) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "IdP and Management data directories must be separate",
                ));
            }
            let engine = Arc::new(
                db::Database::open(std::path::Path::new(directory).join("management.redb"))
                    .map_err(io::Error::other)?,
            );
            management_service::replica::up(&engine)
                .await
                .map_err(io::Error::other)?;
            Some(engine)
        }
        None => None,
    };

    let devices = match &management_engine {
        Some(engine) => Arc::new(DbDeviceRepo::new(Arc::clone(engine))),
        None => Arc::new(DbDeviceRepo::new(Arc::clone(&engine))),
    };

    let router_state = RouterState::new(
        &config.ui_public_uri,
        &config.api_public_uri,
        Arc::clone(&engine),
        Arc::clone(&oauth2_service),
        devices,
        device_identity,
    )
    .with_service_audience(service_audience);
    let router_state = match control_plane {
        Some(control_plane) => router_state.with_hosted_control_plane(control_plane),
        None => router_state,
    };

    let local_permissions = match local_permissions {
        Some(local) => Some(local),
        None => match &management_engine {
            Some(engine) => Some(LocalPermissions::new(Arc::clone(engine))),
            None => None,
        },
    };

    let router_state = match local_permissions {
        Some(local) => router_state.with_local_permissions(local),
        None => router_state,
    };
    let router_state = match remote_permissions {
        Some(remote) => router_state.with_management_permissions(remote),
        None => router_state,
    };

    router_state
        .pairing_acceptance
        .bind(Arc::new(TimedPairingAcceptanceController::new(
            server,
            Duration::from_secs(config.pairing.accepting_timeout_seconds),
        )))
        .map_err(io::Error::other)?;

    let bootstrap_protocol = BootstrapProtocolHandler;
    let router = openapi_router(router_state.clone(), config.server.prefix())
        .split_for_parts()
        .0;

    Ok(IdpRuntime {
        router,
        bootstrap_protocol,

        users: DbUserRepo::new(engine, config.password.clone()),
        join_state: router_state,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use idp_model::model::Id;
    use idp_service::replica::DbUserRepo;
    use iroh::SecretKey;
    use management_service::DeviceRepo;

    use super::IdpRuntime;
    use crate::{BootstrapProtocolHandler, node_join_tests::approval_fixture};

    #[tokio::test]
    async fn approved_runtime_endpoints_need_device_approval_not_signer_enrollment() {
        let fixture = approval_fixture().await;
        let runtime = IdpRuntime {
            router: axum::Router::new(),
            bootstrap_protocol: BootstrapProtocolHandler,
            users: DbUserRepo::new(Arc::clone(&fixture.state.engine), Default::default()),
            join_state: fixture.state.clone(),
        };
        let endpoint = SecretKey::generate().public();
        assert!(runtime.approved_device_owner(endpoint).await.is_err());
        let device = fixture
            .state
            .devices
            .create(
                fixture.owner.to_string(),
                "Device only".into(),
                endpoint.to_string(),
                String::new(),
                b"runtime-enrollment".to_vec(),
                i64::MAX,
            )
            .await
            .expect("pending Device");
        assert!(runtime.approved_device_owner(endpoint).await.is_err());
        fixture
            .state
            .devices
            .approve(device.id, b"runtime-enrollment")
            .await
            .expect("approve Device")
            .expect("approved Device");
        let identity = runtime
            .approved_device_owner(endpoint)
            .await
            .expect("no signer required");
        assert_eq!(identity.device_id, device.id);
        assert_eq!(identity.owner_subject, fixture.owner.to_string());
        assert_eq!(identity.endpoint_id, endpoint.to_string());
        assert!(
            runtime
                .approved_peer_ids()
                .await
                .expect("mesh admission")
                .contains(&endpoint)
        );
        assert!(
            runtime
                .approved_storage_endpoints()
                .await
                .expect("Storage admission")
                .contains(&endpoint.to_string())
        );
        assert!(
            !fixture
                .state
                .replica_signers
                .trusted_signers(&fixture.issuer, i64::MAX)
                .await
                .expect("independent signer trust")
                .iter()
                .any(|(member, _)| member.member_id == device.id)
        );
        fixture
            .state
            .engine
            .client()
            .translate_and_execute(
                &format!(
                    "UPDATE devices SET owner_subject = '{}' WHERE id = CAST('{}' AS UUID)",
                    Id::now_v7(),
                    device.id
                ),
                &db::SqlTranslator,
            )
            .await
            .expect("learn inactive owner");
        assert!(
            runtime.approved_device_owner(endpoint).await.is_err(),
            "unknown owner cannot admit Device"
        );
        fixture
            .state
            .engine
            .client()
            .translate_and_execute(
                &format!(
                    "UPDATE devices SET owner_subject = '{}' WHERE id = CAST('{}' AS UUID)",
                    fixture.owner, device.id
                ),
                &db::SqlTranslator,
            )
            .await
            .expect("restore active owner");
        assert!(runtime.approved_device_owner(endpoint).await.is_ok());
        assert!(
            fixture
                .state
                .devices
                .revoke_self(&endpoint.to_string())
                .await
                .expect("revoke Device")
        );
        assert!(runtime.approved_device_owner(endpoint).await.is_err());
        assert!(
            !runtime
                .approved_peer_ids()
                .await
                .expect("current mesh admission")
                .contains(&endpoint)
        );
        assert!(
            !runtime
                .approved_storage_endpoints()
                .await
                .expect("current Storage admission")
                .contains(&endpoint.to_string())
        );
        drop(runtime);
        fixture.server.endpoint().close().await;
        let root = fixture.root.clone();
        drop(fixture);
        std::fs::remove_dir_all(root).expect("remove isolated runtime fixture");
    }
}
