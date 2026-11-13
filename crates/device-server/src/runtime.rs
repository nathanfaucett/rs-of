use std::sync::Arc;

use axum::Router;
use db::Client;
use device_service::DeviceEndpointIdentity;
use device_service::HostedControlPlane;
use device_service::replica::DbDeviceRepo;
use device_service::replica::DbSelectionPolicyRepo;
use device_service::replica::OwnerPolicyObservation;
use management_service::ManagementService;
use management_service::replica::DbPermissionRepo;
use management_service::replica::DbRoleRepo;
use of_model::contract::SelectedResource;
use of_model::idp::model::Id;

use crate::{RouterState, router::openapi_router};

pub(crate) async fn replication_policy_from_repo(
    policies: &DbSelectionPolicyRepo,
    device: &DeviceEndpointIdentity,
) -> Result<(bool, Vec<SelectedResource>), String> {
    if device.device_id.is_nil()
        || !device
            .owner_subject
            .parse::<Id>()
            .is_ok_and(|owner| !owner.is_nil())
    {
        return Err("invalid local Device owner binding".into());
    }
    let observation = policies
        .observe_owner_prevalidated(device.device_id, &device.owner_subject)
        .await
        .map_err(|error| error.to_string())?;
    replication_policy_from_observation(observation)
}

pub(crate) fn replication_policy_from_observation(
    observation: OwnerPolicyObservation,
) -> Result<(bool, Vec<SelectedResource>), String> {
    let (policy, selected) = match observation {
        OwnerPolicyObservation::Ready { policy, resources } => (policy, resources),
        OwnerPolicyObservation::PendingCleanup { .. } => {
            return Err("learned Device transfer requires local cleanup".into());
        }
        OwnerPolicyObservation::MissingPolicy => {
            return Err("missing locally trusted Device policy".into());
        }
    };
    Ok((
        policy.admin_allowed,
        selected
            .into_iter()
            .map(|resource| SelectedResource {
                owner_subject: resource.owner_subject,
                application_id: resource.application_id.to_string(),
                kind: resource.kind,
                resource_id: resource.resource_id.to_string(),
            })
            .collect(),
    ))
}

/// Builds the local Device owner without setup or admission.
pub async fn build_local_router(
    data_dir: &std::path::Path,
    port: u16,
    idp_base: &str,
    db: Option<Arc<dyn of_model::db::Database>>,
) -> std::io::Result<Router> {
    let client = embedded_db::local_client(data_dir, "device", db)?;
    let storage_base = format!("http://127.0.0.1:{port}/storage");
    let control = HostedControlPlane::new_with_services(idp_base, &storage_base, idp_base)
        .and_then(|control| {
            control.with_idp_service_client("local-device", "local-password", idp_base)
        })
        .map_err(std::io::Error::other)?;
    build_router(
        client.clone(),
        client,
        &format!("http://127.0.0.1:{port}/device"),
        "/device",
        &storage_base,
        Arc::new(control),
        None,
    )
    .await
    .map_err(std::io::Error::other)
}

/// Composes serving from an already initialized owner database.
pub async fn build_router(
    identity: Client,
    devices: Client,
    api_base_uri: &str,
    prefix: &str,
    storage_audience: &str,
    control_plane: Arc<HostedControlPlane>,
    node_joins: Option<Arc<crate::NodeJoinSource>>,
) -> Result<Router, db::QueryError> {
    let control_plane = Arc::new(
        control_plane
            .as_ref()
            .clone()
            .with_local_devices(Arc::new(DbDeviceRepo::new(devices.clone()))),
    );
    let management_service = Arc::new(ManagementService::new(
        DbPermissionRepo::new(identity.clone()),
        DbRoleRepo::new(identity.clone()),
    ));
    let router_state = RouterState::new(
        api_base_uri,
        management_service,
        Arc::new(DbSelectionPolicyRepo::new(devices)),
        control_plane,
        storage_audience,
    );

    let router: Router = openapi_router(router_state, prefix).into();
    Ok(router.merge(crate::node_join_source::node_join_router(
        node_joins, prefix,
    )))
}

#[cfg(test)]
mod tests {
    use super::replication_policy_from_repo;
    use db::Database;
    use device_service::DeviceEndpointIdentity;
    use device_service::replica::DbSelectionPolicyRepo;
    use device_service::replica::SelectionPolicy;
    use device_service::replica::up;
    use of_model::contract::SelectedResource;
    use of_model::idp::model::Id;
    use std::{fs, sync::Arc};

    async fn replication_policy(
        engine: Arc<Database>,
        device: &DeviceEndpointIdentity,
    ) -> Result<(bool, Vec<SelectedResource>), String> {
        replication_policy_from_repo(&DbSelectionPolicyRepo::new(engine.client()), device).await
    }

    #[tokio::test]
    async fn native_owner_policy_requires_binding_and_rechecks_persisted_restrictions() {
        let root = std::env::temp_dir().join(format!("local-owner-policy-{}", Id::now_v7()));
        fs::create_dir_all(&root).expect("owner policy fixture");
        let path = root.join("devices.redb");
        let device = DeviceEndpointIdentity {
            device_id: Id::now_v7(),
            owner_subject: Id::now_v7().to_string(),
            endpoint_id: "authenticated-by-idp".into(),
        };
        {
            let engine = Arc::new(Database::open(&path).expect("native Management owner"));
            up(&engine).await.expect("owner schema");
            assert!(
                replication_policy(Arc::clone(&engine), &device)
                    .await
                    .is_err(),
                "missing policy cannot fall back to remote approval"
            );
            let policies = DbSelectionPolicyRepo::new(Arc::clone(&engine).client());
            policies
                .set_prevalidated(SelectionPolicy {
                    device_id: device.device_id,
                    owner_subject: device.owner_subject.clone(),
                    application_id: None,
                    selected_kind: None,
                    selected_id: None,
                    admin_allowed: true,
                })
                .await
                .expect("persist empty selection");
            let (allowed, resources) = replication_policy(Arc::clone(&engine), &device)
                .await
                .expect("approved local policy");
            assert!(allowed);
            assert!(resources.is_empty());
            let observation = policies
                .observe_owner_prevalidated(device.device_id, &device.owner_subject)
                .await
                .expect("coherent ready observation");
            assert!(
                matches!(observation, device_service::replica::OwnerPolicyObservation::Ready { policy, resources } if policy.admin_allowed && resources.is_empty())
            );
            let forged = DeviceEndpointIdentity {
                owner_subject: Id::now_v7().to_string(),
                ..device.clone()
            };
            assert!(
                replication_policy(Arc::clone(&engine), &forged)
                    .await
                    .is_err()
            );
            assert!(
                policies
                    .local_cleanup_pending(device.device_id)
                    .await
                    .expect("owner mismatch retains retirement fence")
            );
            assert!(
                policies
                    .complete_owner_cleanup_prevalidated(
                        device.device_id,
                        &device.owner_subject,
                        &device.owner_subject
                    )
                    .await
                    .expect("finish original-owner return cleanup")
            );
            policies
                .set_admin_allowed_prevalidated(device.device_id, &device.owner_subject, false)
                .await
                .expect("learn restriction");
            let (allowed, resources) = replication_policy(Arc::clone(&engine), &device)
                .await
                .expect("current restriction");
            assert!(!allowed);
            assert!(resources.is_empty());
        }
        let engine = Arc::new(Database::open(&path).expect("reopen owner offline"));
        let (allowed, resources) = replication_policy(Arc::clone(&engine), &device)
            .await
            .expect("persisted restriction");
        assert!(!allowed);
        assert!(resources.is_empty());
        drop(engine);
        fs::remove_dir_all(root).expect("remove owner policy fixture");
    }
}
