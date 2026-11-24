use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use iroh::SecretKey;
use of_model::{
    contract::replication::{
        MANAGEMENT_REPLICATION_ADMIT_SCOPE, MANAGEMENT_REPLICATION_READ_SCOPE,
    },
    device::contract::{
        device_pairing::DevicePairingApprovalPayload,
        device_self_revocation::device_self_revocation_payload,
    },
    id::Id,
    idp::contract::{
        permission_evaluation::{INSTALLATION_POLICY_ID, IdentityAction},
        service_scopes::MANAGEMENT_DEVICE_LOOKUP_SCOPE,
    },
};
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};

use super::{
    OwnerFixture, assert_status, exchange_storage, exchange_token, frontend_token, grant,
    response_json,
};

#[tokio::test]
async fn signed_pairing_and_revocation_do_not_replace_replication_policy() {
    let fixture = OwnerFixture::start().await;
    let OwnerFixture {
        client,
        base,
        management,
        ..
    } = &fixture;
    let idp = format!("{base}/idp");
    let device = format!("{base}/device");
    let storage = format!("{base}/storage");
    let admin = fixture.sign_in("admin@local.test", "password").await;
    let actor = fixture.subject(&admin).await;
    let idp_token = exchange_token(client, base, "local-login", &admin, &idp, None).await;
    let frontend = frontend_token(
        client,
        base,
        &admin,
        "app",
        "http://localhost:5173/callback",
    )
    .await;
    let frontend = text(&frontend, "access_token");
    let actor_token = exchange_token(client, base, "app", frontend, &device, None).await;
    grant(
        management,
        INSTALLATION_POLICY_ID,
        actor,
        "device-lifecycle-fixture",
        &[
            IdentityAction::DevicesEnroll.permission(),
            IdentityAction::DeviceRestrictionsUpdate.permission(),
        ],
    )
    .await;

    let first_key = SecretKey::generate().public().to_string();
    let first = enroll(client, &device, &actor_token, &first_key).await;
    let first_id = text(&first, "id");
    let second_secret = SecretKey::generate();
    let second_key = second_secret.public().to_string();
    let (pending, proof) = pairing(client, &device, &actor_token, &first_key, &second_secret).await;
    let second_id = text(&pending, "id");
    let approval = format!("{device}/devices/pairings/{second_id}/approve");
    let approve = |token: &str| client.post(&approval).bearer_auth(token).json(&proof);
    let pending_list = || {
        client
            .get(format!("{device}/devices/enrollments"))
            .bearer_auth(&actor_token)
    };
    let before = response_json(pending_list()).await;
    assert!(
        before
            .as_array()
            .expect("pending Device list")
            .contains(&pending)
    );

    let other = fixture.sign_in("other-user", "other-password").await;
    let other_token = exchange_token(client, base, "local-login", &other, &device, None).await;
    assert_status(approve(&other_token), StatusCode::FORBIDDEN).await;
    assert_eq!(response_json(pending_list()).await, before);
    let approved = response_json(approve(&actor_token)).await;
    assert_eq!(approved["id"], second_id);
    assert_eq!(approved["publicKey"], second_key);
    assert_eq!(approved["state"], "approved");
    let storage_token = replication_client(client, &idp, &storage, &idp_token).await;
    assert_revocation_blocks_replication(
        &fixture,
        &actor_token,
        actor,
        frontend,
        &storage_token,
        [
            (first_id, first_key.as_str()),
            (second_id, second_key.as_str()),
        ],
    )
    .await;

    assert_signed_self_revocation(client, &device, &actor_token, &first_key, &storage_token).await;
}

async fn assert_signed_self_revocation(
    client: &Client,
    device: &str,
    actor_token: &str,
    first_key: &str,
    storage_token: &str,
) {
    let self_secret = SecretKey::generate();
    let public_key = self_secret.public().to_string();
    let (pending, proof) = pairing(client, device, actor_token, first_key, &self_secret).await;
    let self_id = text(&pending, "id");
    let approved = response_json(
        client
            .post(format!("{device}/devices/pairings/{self_id}/approve"))
            .bearer_auth(actor_token)
            .json(&proof),
    )
    .await;
    assert_eq!(approved["state"], "approved");
    let revoke_self = |payload: &str| {
        client
            .post(format!("{device}/devices/revoke-self"))
            .json(&json!({
                "publicKey": public_key, "signature": signature(&self_secret, payload),
            }))
    };
    let devices = || {
        client
            .get(format!("{device}/devices"))
            .bearer_auth(actor_token)
    };
    let before = response_json(devices()).await;
    assert_status(revoke_self("wrong purpose"), StatusCode::FORBIDDEN).await;
    assert_eq!(response_json(devices()).await, before);
    let lookup = || {
        client
            .get(format!("{device}/devices/endpoints/{public_key}"))
            .bearer_auth(storage_token)
    };
    assert_eq!(response_json(lookup()).await["deviceId"], self_id);
    assert_status(
        revoke_self(&device_self_revocation_payload(&public_key)),
        StatusCode::OK,
    )
    .await;
    assert_status(lookup(), StatusCode::NOT_FOUND).await;
}

#[tokio::test]
#[ignore = "IdP active-User lookup accepts blocked Users; run explicitly until fixed"]
async fn blocked_user_with_an_existing_device_token_and_enroll_grant_cannot_approve() {
    let fixture = OwnerFixture::start().await;
    let OwnerFixture {
        client,
        base,
        management,
        ..
    } = &fixture;
    let idp = format!("{base}/idp");
    let device = format!("{base}/device");
    let admin = fixture.sign_in("admin@local.test", "password").await;
    let actor = fixture.subject(&admin).await;
    let idp_token = exchange_token(client, base, "local-login", &admin, &idp, None).await;
    let actor_token = exchange_token(client, base, "local-login", &admin, &device, None).await;
    grant(
        management,
        INSTALLATION_POLICY_ID,
        actor,
        "block-fixture",
        &[IdentityAction::UsersBlock.permission()],
    )
    .await;
    let first_key = SecretKey::generate().public().to_string();
    enroll(client, &device, &actor_token, &first_key).await;
    let (pending, proof) = pairing(
        client,
        &device,
        &actor_token,
        &first_key,
        &SecretKey::generate(),
    )
    .await;
    let other = fixture.sign_in("other-user", "other-password").await;
    let other_id = fixture.subject(&other).await;
    let other_token = exchange_token(client, base, "local-login", &other, &device, None).await;
    grant(
        management,
        INSTALLATION_POLICY_ID,
        other_id,
        "pairing-fixture",
        &[IdentityAction::DevicesEnroll.permission()],
    )
    .await;
    assert_status(
        client
            .post(format!("{idp}/users/{other_id}/block"))
            .bearer_auth(idp_token)
            .json(&json!({"blocked": true})),
        StatusCode::OK,
    )
    .await;

    let id = text(&pending, "id");
    let response = client
        .post(format!("{device}/devices/pairings/{id}/approve"))
        .bearer_auth(other_token)
        .json(&proof)
        .send()
        .await
        .expect("blocked actor response");
    let status = response.status();
    let after = response_json(
        client
            .get(format!("{device}/devices/enrollments"))
            .bearer_auth(actor_token),
    )
    .await;
    assert!(
        matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN)
            && after
                .as_array()
                .expect("pending Device list")
                .contains(&pending),
        "blocked actor must not approve Device {id}; HTTP {status}, pending list: {after}"
    );
}

async fn assert_revocation_blocks_replication(
    fixture: &OwnerFixture,
    actor_token: &str,
    actor: Id,
    frontend: &str,
    storage_token: &str,
    [(first_id, first_key), (second_id, second_key)]: [(&str, &str); 2],
) {
    let OwnerFixture {
        client,
        base,
        management,
        ..
    } = fixture;
    let device = format!("{base}/device");
    let storage = format!("{base}/storage");
    let write = exchange_storage(client, base, "app", frontend, "storage:databases", "write").await;
    let read = exchange_storage(client, base, "app", frontend, "storage:databases", "read").await;
    let resource = response_json(
        client
            .post(format!("{storage}/databases"))
            .bearer_auth(write)
            .json(&json!({"name": "Replication fixture"})),
    )
    .await;
    let resource_id = text(&resource, "id");
    let url = format!("{storage}/databases/{resource_id}");
    let details = response_json(client.get(&url).bearer_auth(&read)).await;
    let app = text(&details, "applicationId")
        .parse::<Id>()
        .expect("Application ID");
    let admission = || {
        client
            .post(format!("{device}/replication/admission"))
            .bearer_auth(storage_token)
            .json(&json!({
                "sourceEndpointId": first_key, "targetEndpointId": second_key, "applicationId": app,
                "kind": "database", "resourceId": resource_id, "operation": "synchronize",
            }))
    };
    let target_selections = || {
        client
            .get(format!(
                "{device}/replication/devices/{second_key}/selections"
            ))
            .bearer_auth(storage_token)
    };
    let selection = |id: &str, token: &str| {
        client.put(format!("{device}/devices/{id}/selection")).bearer_auth(actor_token).json(&json!({
        "applicationId": app, "id": resource_id, "kind": "database", "storageAccessToken": token,
    }))
    };
    assert_status(admission(), StatusCode::FORBIDDEN).await;
    assert_status(selection(second_id, &read), StatusCode::FORBIDDEN).await;
    grant(
        management,
        app,
        actor,
        "replication-selection-fixture",
        &["devices.select"],
    )
    .await;
    assert_status(selection(second_id, actor_token), StatusCode::FORBIDDEN).await;
    // An explicit restriction record makes the service-only Selection listing available.
    assert_status(
        client
            .put(format!("{device}/devices/{second_id}/restriction"))
            .bearer_auth(actor_token)
            .json(&json!({"adminAllowed": true})),
        StatusCode::NO_CONTENT,
    )
    .await;
    let empty = response_json(target_selections()).await;
    assert_eq!(empty["deviceId"], second_id);
    assert_eq!(empty["resources"], json!([]));
    assert_status(selection(first_id, &read), StatusCode::NO_CONTENT).await;
    assert_status(admission(), StatusCode::FORBIDDEN).await;
    assert_status(selection(second_id, &read), StatusCode::NO_CONTENT).await;
    assert_status(admission(), StatusCode::NO_CONTENT).await;

    let revoke = || {
        client
            .delete(format!("{device}/devices/{second_id}"))
            .bearer_auth(actor_token)
    };
    assert_status(revoke(), StatusCode::FORBIDDEN).await;
    assert_status(admission(), StatusCode::NO_CONTENT).await;
    grant(
        management,
        INSTALLATION_POLICY_ID,
        actor,
        "revoke-fixture",
        &[IdentityAction::DevicesRevoke.permission()],
    )
    .await;
    assert_status(revoke(), StatusCode::NO_CONTENT).await;
    assert_status(
        client
            .get(format!("{device}/devices/endpoints/{second_key}"))
            .bearer_auth(storage_token),
        StatusCode::NOT_FOUND,
    )
    .await;
    // The admission route reports an unavailable binding after learned revocation.
    assert_status(admission(), StatusCode::SERVICE_UNAVAILABLE).await;
    assert_status(selection(second_id, &read), StatusCode::FORBIDDEN).await;
    assert_eq!(
        response_json(client.get(url).bearer_auth(read)).await,
        details
    );
}

async fn enroll(client: &Client, device: &str, token: &str, public_key: &str) -> Value {
    let first = response_json(
        client
            .post(format!("{device}/devices/enrollments"))
            .bearer_auth(token)
            .json(&json!({
                "name": "Accepting Device", "publicKey": public_key, "address": "127.0.0.1:23456",
            })),
    )
    .await;
    assert_eq!(first["state"], "approved");
    first
}

async fn pairing(
    client: &Client,
    device: &str,
    token: &str,
    accepting_key: &str,
    secret: &SecretKey,
) -> (Value, Value) {
    let pairings = format!("{device}/devices/pairings");
    let pending = response_json(client.post(&pairings).bearer_auth(token).json(&json!({
        "name": "Paired Device", "publicKey": secret.public().to_string(),
        "acceptingPublicKey": accepting_key, "address": "127.0.0.1:23457",
    })))
    .await;
    assert_eq!(pending["state"], "pending");
    let id = text(&pending, "id");
    let payload: DevicePairingApprovalPayload = serde_json::from_value(
        response_json(
            client
                .get(format!("{pairings}/{id}/payload"))
                .bearer_auth(token),
        )
        .await,
    )
    .expect("string pairing payload");
    (
        pending,
        json!({"signature": signature(secret, &payload.payload)}),
    )
}

async fn replication_client(client: &Client, idp: &str, storage: &str, admin: &str) -> String {
    let scopes = [
        MANAGEMENT_DEVICE_LOOKUP_SCOPE,
        MANAGEMENT_REPLICATION_READ_SCOPE,
        MANAGEMENT_REPLICATION_ADMIT_SCOPE,
    ];
    let registration = response_json(client.post(format!("{idp}/oauth2/register")).bearer_auth(admin).json(&json!({
        "application": {"uri": idp}, "client_name": "Test Storage service",
        "client_type": "confidential", "profile": "web_application", "allowed_grant_types": ["client_credentials"],
        "allowed_audiences": [storage], "allowed_scopes": scopes, "token_endpoint_auth_method": "client_secret_post",
    }))).await;
    let token = response_json(client.post(format!("{idp}/oauth2/token")).form(&[
        ("grant_type", "client_credentials"),
        ("audience", storage),
        ("scope", &scopes.join(" ")),
        ("client_id", text(&registration, "client_id")),
        ("client_secret", text(&registration, "client_secret")),
    ]))
    .await;
    text(&token, "access_token").to_owned()
}

fn signature(secret: &SecretKey, payload: &str) -> String {
    URL_SAFE_NO_PAD.encode(secret.sign(payload.as_bytes()).to_bytes())
}

fn text<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field].as_str().expect("required string field")
}
