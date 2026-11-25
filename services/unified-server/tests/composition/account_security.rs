use super::{
    INSTALLATION_POLICY_ID, IdentityAction, OwnerFixture, StatusCode, Value, assert_status,
    exchange_token, grant, json, response_json,
};

#[tokio::test]
async fn account_security_requires_each_installation_permission() {
    let fixture = OwnerFixture::start().await;
    let idp = format!("{}/idp", fixture.base);
    let login = fixture.sign_in("admin@local.test", "password").await;
    let actor = fixture.subject(&login).await;
    let target = fixture.sign_in("other-user", "other-password").await;
    let target = fixture.subject(&target).await;
    let token = exchange_token(
        &fixture.client,
        &fixture.base,
        "local-login",
        &login,
        &idp,
        None,
    )
    .await;
    let url = format!("{idp}/users/{target}");
    let update = |action: &str, body: Value, bearer: &str| {
        fixture
            .client
            .post(format!("{url}/{action}"))
            .bearer_auth(bearer)
            .json(&body)
    };
    let read = || response_json(fixture.client.get(&url).bearer_auth(&token));
    let mfa = |enable| json!({"method": "totp", "enable": enable});
    let block = |blocked| json!({"blocked": blocked});

    assert_status(update("mfa", mfa(true), &token), StatusCode::FORBIDDEN).await;
    assert_status(update("block", block(true), &token), StatusCode::FORBIDDEN).await;
    let state = read().await;
    assert_eq!(state["mfa_totp_enabled"], false);
    assert_eq!(state["blocked"], false);

    grant(
        &fixture.management,
        INSTALLATION_POLICY_ID,
        actor,
        "account-block",
        &[IdentityAction::UsersBlock.permission()],
    )
    .await;
    assert_status(update("mfa", mfa(true), &token), StatusCode::FORBIDDEN).await;
    assert_status(update("block", block(true), &login), StatusCode::FORBIDDEN).await;
    assert_status(update("block", block(true), &token), StatusCode::OK).await;
    assert_eq!(read().await["blocked"], true);
    assert_status(update("block", block(false), &token), StatusCode::OK).await;
    assert_eq!(read().await["blocked"], false);

    grant(
        &fixture.management,
        INSTALLATION_POLICY_ID,
        actor,
        "account-mfa",
        &[IdentityAction::UsersMfa.permission()],
    )
    .await;
    assert_status(update("mfa", mfa(true), &token), StatusCode::OK).await;
    assert_eq!(read().await["mfa_totp_enabled"], true);
    assert_status(update("mfa", mfa(false), &token), StatusCode::OK).await;
    assert_eq!(read().await["mfa_totp_enabled"], false);
}
