use idp_model::{
    contract::{ErrorCode, IntrospectionResponse},
    model::Id,
};
use iroh::EndpointId;
use management_service::DeviceRepo;
use management_service::{DeviceEndpointIdentity, DeviceState};

use crate::{IdpRuntime, RouterState, authorize_bearer_any_principal};

pub async fn authorize_management_bearer(
    state: &RouterState,
    bearer: &str,
) -> Result<IntrospectionResponse, String> {
    let inspected = authorize_bearer_any_principal(state, bearer)
        .await
        .map_err(|error| match error.error {
            ErrorCode::NotAuthorized => "actor token was rejected by IdP".to_owned(),
            _ => "IdP token validation is unavailable".to_owned(),
        })?;
    let application_id = state
        .oauth2_service
        .application_id_for_client(&inspected.claims.client_id)
        .await
        .map_err(|_| "IdP token validation is unavailable".to_owned())?;
    Ok(IntrospectionResponse {
        claims: inspected.claims,
        application_id: application_id.to_string(),
    })
}

pub async fn approved_management_endpoint(
    runtime: &IdpRuntime,
    endpoint_id: &str,
) -> Result<DeviceEndpointIdentity, String> {
    let endpoint = endpoint_id
        .parse::<EndpointId>()
        .map_err(|_| "invalid endpoint ID".to_owned())?;
    if endpoint.to_string() != endpoint_id {
        return Err("invalid endpoint ID".into());
    }
    runtime
        .approved_device_owner(endpoint)
        .await
        .map_err(|error| error.to_string())
}

pub async fn approved_management_device(
    state: &RouterState,
    runtime: &IdpRuntime,
    device_id: Id,
) -> Result<DeviceEndpointIdentity, String> {
    if device_id.is_nil() {
        return Err("invalid Device ID".into());
    }
    let device = state
        .devices
        .list()
        .await
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|device| device.id == device_id && device.state == DeviceState::Approved)
        .ok_or_else(|| "IdP identity was not found".to_owned())?;
    let identity = approved_management_endpoint(runtime, &device.public_key).await?;
    if identity.device_id != device_id || identity.owner_subject != device.owner_subject {
        return Err("IdP returned a mismatched Device identity".into());
    }
    Ok(identity)
}

#[cfg(test)]
mod tests {
    use idp_service::repo::ClientRepo;
    use management_service::HostedControlPlane;

    use crate::node_join_tests::{approval_fixture, user_token};

    use super::authorize_management_bearer;

    #[tokio::test]
    async fn local_management_bearer_rechecks_current_client_registration() {
        let fixture = approval_fixture().await;
        let token = user_token(&fixture, "idp-services").await;
        let state = fixture.state.clone();
        let control = HostedControlPlane::new_with_services(
            "http://127.0.0.1:1",
            "http://127.0.0.1:1",
            &fixture.issuer,
        )
        .expect("unused HTTP endpoints")
        .with_local_idp(
            move |token| {
                let state = state.clone();
                async move { authorize_management_bearer(&state, &token).await }
            },
            |_| async { Err("unused endpoint lookup".into()) },
            |_| async { Err("unused Device lookup".into()) },
        );
        let (claims, application_id) = control
            .validate_actor_token(&token)
            .await
            .expect("real local bearer without Management service credentials");
        assert_eq!(claims.sub, fixture.owner.to_string());
        assert_eq!(claims.client_id, "human-client");
        assert!(!application_id.is_nil());
        assert!(control.validate_actor_token("invalid-token").await.is_err());
        fixture
            .state
            .oauth2_service
            .client_repo
            .delete_client_by_client_id("human-client")
            .await
            .expect("learn registration deletion");
        assert!(control.validate_actor_token(&token).await.is_err());
        drop(control);
        fixture.server.endpoint().close().await;
        let root = fixture.root.clone();
        drop(fixture);
        std::fs::remove_dir_all(root).expect("remove isolated owner fixture");
    }
}
