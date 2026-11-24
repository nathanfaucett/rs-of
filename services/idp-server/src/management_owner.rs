use idp_service::ofdb::LocalIdentity;
use of_model::id::Id;
use of_model::idp::contract::error_code::ErrorCode;
use of_model::idp::contract::introspection_response::IntrospectionResponse;

use crate::router::{
    middleware::authorization::authorize_bearer_any_principal, state::RouterState,
};

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

    Ok(IntrospectionResponse {
        claims: inspected.claims,
        application_id: inspected.application_id.to_string(),
    })
}

/// Checks current `IdP` User status independently of token validation.
///
/// # Errors
/// Returns an error if the User ID is invalid, the User is absent or blocked,
/// or current status cannot be determined.
pub async fn active_management_user(state: &RouterState, user_id: Id) -> Result<(), String> {
    if user_id.is_nil() {
        return Err("invalid User ID".into());
    }
    match LocalIdentity::new(state.engine.clone())
        .is_active_user(user_id)
        .await
    {
        Ok(true) => Ok(()),
        Ok(false) | Err(_) => Err("IdP User is not currently active".into()),
    }
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request};
    use device_service::hosted_control_plane::HostedControlPlane;
    use idp_service::{oauth2::verify_access_jwt, repo::client_repo::ClientRepo};
    use of_model::idp::contract::jwks::Jwks;
    use tower::ServiceExt;

    use crate::owner_test_fixture::{owner_fixture, user_token};

    use crate::management_owner::authorize_management_bearer;

    #[tokio::test]
    async fn local_management_bearer_retains_signed_application_after_client_deletion() {
        let fixture = owner_fixture().await;
        let token = user_token(&fixture, "idp-services").await;
        let router: axum::Router = crate::openapi_router(fixture.state.clone(), "/idp").into();
        let request = Request::builder()
            .uri("/idp/.well-known/jwks.json")
            .body(Body::empty())
            .expect("issuer JWKS request");
        let response = router.oneshot(request).await.expect("issuer JWKS response");
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let jwks: Jwks = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 16 * 1024)
                .await
                .expect("read public keys"),
        )
        .expect("parse public keys");
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
        let (after, after_application) = control
            .validate_actor_token(&token)
            .await
            .expect("an unexpired JWT still validates");
        assert_eq!(after.sub, claims.sub);
        assert_eq!(after_application, application_id);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("valid test clock")
            .as_secs() as i64;
        let verified = verify_access_jwt(&jwks, &fixture.issuer, "idp-services", &token, now)
            .expect("verify using only the retained JWKS");
        assert_eq!(verified.application_id, application_id);
        assert_eq!(verified.standard_claims.sub, fixture.owner.to_string());
        drop(control);
        fixture.server.endpoint().close().await;
        let root = fixture.root.clone();
        drop(fixture);
        std::fs::remove_dir_all(root).expect("remove isolated owner fixture");
    }
}
