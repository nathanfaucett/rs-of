use idp_model::{
    contract::{ErrorCode, IntrospectionResponse},
    model::Id,
};

use crate::{RouterState, authorize_bearer_any_principal};

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

pub async fn active_management_user(state: &RouterState, user_id: Id) -> Result<(), String> {
    if user_id.is_nil() {
        return Err("invalid User ID".into());
    }
    state
        .oauth2_service
        .find_user_info(user_id)
        .await
        .map(|_| ())
        .map_err(|_| "IdP User is not currently active".into())
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
            |_| async { Ok(()) },
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
