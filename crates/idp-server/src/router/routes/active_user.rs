use crate::{RouterState, authorize_bearer_client};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
};
use idp_model::{
    contract::{ErrorCode, ErrorResponse, IDP_USER_LOOKUP_SCOPE},
    model::Id,
};

#[utoipa::path(get, path = "/users/{user_id}/active", params(("user_id" = String, Path)),
    responses((status = 204, description = "Current conflict-clear active User"), (status = 401, description = "Invalid Client token"), (status = 403, description = "Missing exact service permission"), (status = 404, description = "User is not active")), security(("authorization" = [])))]
pub(crate) async fn active_user(
    State(state): State<RouterState>,
    Path(user_id): Path<Id>,
    headers: HeaderMap,
) -> Result<StatusCode, ErrorResponse> {
    let token = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ErrorResponse::new(ErrorCode::NotAuthorized))?;
    let actor = authorize_bearer_client(&state, token).await?;
    if actor.claims.aud != state.service_audience
        || !actor
            .claims
            .scope
            .iter()
            .any(|scope| scope == IDP_USER_LOOKUP_SCOPE)
    {
        return Err(ErrorResponse::new(ErrorCode::AccessDenied));
    }
    crate::management_owner::active_management_user(&state, user_id)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::NotFound))?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use idp_model::contract::{
        ApplicationRegistration, ClientCredentialsGrantRequest, ClientProfile, ClientRegistration,
        ClientType, GrantType, OAuth2ClientAuth, TokenEndpointAuthMethod, TokenRequest,
    };
    use idp_service::repo::{ClientRepo, UserRepo};
    use tower::ServiceExt;

    #[tokio::test]
    async fn active_user_route_requires_client_exact_scope_audience_and_current_user() {
        let fixture = crate::node_join_tests::approval_fixture().await;
        fixture
            .state
            .oauth2_service
            .client_repo
            .create_client(ClientRegistration {
                application: ApplicationRegistration {
                    name: Some("Management User lookup".into()),
                    uri: "https://management-user-lookup.test".into(),
                    description: None,
                },
                client_id: Some("user-lookup".into()),
                client_secret: Some("lookup-secret".into()),
                client_id_issued_at: None,
                client_secret_expires_at: None,
                client_name: "User lookup".into(),
                client_uri: None,
                logo_uri: None,
                contacts: vec![],
                terms_of_service_uri: None,
                policy_uri: None,
                client_type: ClientType::Confidential,
                profile: ClientProfile::Web,
                redirect_uris: vec![],
                allowed_grant_types: vec![GrantType::ClientCredentials],
                response_types: vec![],
                allowed_scopes: vec!["idp.user.lookup".into(), "idp.token.validate".into()],
                allowed_audiences: vec!["idp-services".into(), "other-services".into()],
                token_endpoint_auth_method: TokenEndpointAuthMethod::ClientSecretPost,
                software_statement: None,
                software_id: None,
                software_version: None,
            })
            .await
            .expect("scoped Management Client");
        let router: axum::Router = crate::openapi_router(fixture.state.clone(), "/idp").into();
        let call = |token: String, user| {
            let router = router.clone();
            async move {
                router
                    .oneshot(
                        Request::builder()
                            .uri(format!("/idp/users/{user}/active"))
                            .header("authorization", format!("Bearer {token}"))
                            .body(Body::empty())
                            .expect("User lookup"),
                    )
                    .await
                    .expect("IdP response")
                    .status()
            }
        };
        let user_token = crate::node_join_tests::user_token(&fixture, "idp-services").await;
        assert!(!call(user_token, fixture.owner).await.is_success());
        let mut authorized = None;
        for (scope, audience, expected) in [
            ("idp.token.validate", "idp-services", StatusCode::FORBIDDEN),
            ("idp.user.lookup", "other-services", StatusCode::FORBIDDEN),
            ("idp.user.lookup", "idp-services", StatusCode::NO_CONTENT),
        ] {
            let token = fixture
                .state
                .oauth2_service
                .token(
                    TokenRequest::ClientCredentials(ClientCredentialsGrantRequest {
                        client_id: "user-lookup".into(),
                        client_secret: "lookup-secret".into(),
                        scope: Some(scope.into()),
                        audience: Some(audience.into()),
                        resource: None,
                    }),
                    Some(OAuth2ClientAuth {
                        client_id: "user-lookup".into(),
                        client_secret: Some("lookup-secret".into()),
                        method: TokenEndpointAuthMethod::ClientSecretPost,
                    }),
                )
                .await
                .expect("service token")
                .access_token
                .0;
            assert_eq!(call(token.clone(), fixture.owner).await, expected);
            if expected == StatusCode::NO_CONTENT {
                authorized = Some(token);
            }
        }
        let token = authorized.expect("authorized service token");
        assert_eq!(
            call(token.clone(), idp_model::model::Id::now_v7()).await,
            StatusCode::NOT_FOUND
        );
        fixture
            .state
            .oauth2_service
            .user_repo
            .delete_user_by_id(fixture.owner)
            .await
            .expect("learn User deletion");
        assert_eq!(call(token, fixture.owner).await, StatusCode::NOT_FOUND);
        drop(router);
        fixture.server.endpoint().close().await;
        let root = fixture.root.clone();
        drop(fixture);
        std::fs::remove_dir_all(root).expect("isolated owner cleanup");
    }
}
