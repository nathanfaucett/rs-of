use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header::LOCATION},
};
use idp_model::contract::AuthorizationRequest;
use idp_service::{
    PasswordConfig,
    oauth2::{OAuth2Config, OAuth2Service},
    replica::{
        DbApplicationRepo, DbClientRepo, DbKeyRepo, DbOAuth2AuthorizationCodeRepo,
        DbOAuth2RefreshTokenRepo, DbOAuth2UserConsentRepo, DbUserRepo,
    },
    repo::{KeyService, PrivateKeyKeyringRepo},
};
use reqwest::Url;
use tower::ServiceExt;

fn router() -> Router {
    let database = Arc::new(db::Database::in_memory());
    let engine = database.client();
    let keys = Arc::new(KeyService::new(
        DbKeyRepo::new(engine.clone()),
        PrivateKeyKeyringRepo::new_with_store(
            "authorize-ui",
            keyring_core::mock::Store::new().expect("isolated credentials"),
        ),
        "authorize-ui",
    ));
    let service = OAuth2Service::new(
        DbApplicationRepo::new(engine.clone()),
        DbClientRepo::new(engine.clone(), Arc::clone(&keys)),
        DbOAuth2AuthorizationCodeRepo::new(engine.clone()),
        DbOAuth2RefreshTokenRepo::new(engine.clone()),
        DbUserRepo::new(engine.clone(), PasswordConfig::default()),
        DbOAuth2UserConsentRepo::new(engine.clone()),
        keys,
        OAuth2Config::default(),
    );
    let state = crate::RouterState::new_oauth(
        "https://ui.test/prefix/",
        "https://idp.test/idp",
        engine,
        Arc::new(service),
    );
    crate::openapi_router(state, "/idp").into()
}

fn onboarding_url() -> Url {
    let mut url = Url::parse("https://idp.test/idp/oauth2/auth").expect("authorization URL");
    url.query_pairs_mut().extend_pairs([
        ("response_type", "code"),
        ("application_id", "password-manager.aicacia.com"),
        ("registration", r#"{"application":{"name":"Aicacia Password Manager","uri":"password-manager.aicacia.com"},"client_name":"Password Manager"}"#),
        ("redirect_uri", "https://password-manager.aicacia.com/callback"),
        ("state", "state + / & = %"),
        ("nonce", "nonce-for-onboarding"),
        ("scope", "openid profile storage"),
        ("code_challenge", "pkce-challenge"),
        ("code_challenge_method", "S256"),
        ("extension", "first"),
        ("extension", "second"),
    ]);
    url
}

async fn get(router: &Router, url: &Url) -> axum::response::Response {
    router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("{}?{}", url.path(), url.query().expect("query")))
                .body(Body::empty())
                .expect("UI entry request"),
        )
        .await
        .expect("real router response")
}

#[tokio::test]
async fn authorize_ui_entry_accepts_onboarding_without_client_id_and_preserves_raw_query() {
    let router = router();
    let url = onboarding_url();
    let response = get(&router, &url).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response
            .headers()
            .get(LOCATION)
            .expect("UI location")
            .to_str()
            .expect("location text"),
        format!(
            "https://ui.test/prefix/authorize?{}",
            url.query().expect("original query")
        )
    );
    let mut existing = Url::parse("https://idp.test/idp/oauth2/auth?response_type=code&client_id=registered-client&state=preserved%20state").expect("existing Client URL");
    assert_eq!(
        get(&router, &existing).await.status(),
        StatusCode::SEE_OTHER
    );
    existing
        .query_pairs_mut()
        .append_pair("redirect_uri", "aicacia-password-manager://callback");
    assert_eq!(
        get(&router, &existing).await.status(),
        StatusCode::SEE_OTHER
    );
    let mut native = onboarding_url();
    let pairs: Vec<_> = native
        .query_pairs()
        .filter(|(key, _)| key != "redirect_uri")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    native
        .query_pairs_mut()
        .clear()
        .extend_pairs(pairs)
        .append_pair("redirect_uri", "aicacia-password-manager://callback");
    assert_eq!(get(&router, &native).await.status(), StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn authorize_ui_entry_denies_malformed_onboarding_without_redirecting() {
    let router = router();
    for (field, replacement) in [
        ("application_id", None),
        ("application_id", Some(" ")),
        ("application_id", Some("another.application")),
        ("registration", None),
        ("registration", Some("not-json")),
        ("registration", Some("null")),
        ("registration", Some("[]")),
        ("registration", Some("{}")),
        (
            "registration",
            Some(
                r#"{"application":{"uri":"password-manager.aicacia.com"},"client_id":"caller-chosen"}"#,
            ),
        ),
        ("redirect_uri", None),
        ("redirect_uri", Some("not a URL")),
        ("redirect_uri", Some("javascript:alert(1)")),
        ("response_type", None),
        ("response_type", Some("invalid")),
        ("response_type", Some("token")),
        ("code_challenge_method", Some("invalid")),
        ("client_id", Some("")),
    ] {
        let mut url = onboarding_url();
        let pairs: Vec<_> = url
            .query_pairs()
            .filter(|(key, _)| key != field)
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        url.query_pairs_mut().clear().extend_pairs(pairs);
        if let Some(value) = replacement {
            url.query_pairs_mut().append_pair(field, value);
        }
        let response = get(&router, &url).await;
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "{field}: {replacement:?}"
        );
        assert!(!response.headers().contains_key(LOCATION));
    }
}

#[test]
fn authorize_json_code_issuance_still_requires_client_id() {
    let mut request = serde_json::json!({"response_type":"code", "application_id":"password-manager.aicacia.com", "registration":"{}", "redirect_uri":"https://password-manager.aicacia.com/callback"});
    assert!(serde_json::from_value::<AuthorizationRequest>(request.clone()).is_err());
    request["client_id"] = "server-issued-client".into();
    assert!(serde_json::from_value::<AuthorizationRequest>(request).is_ok());
}
