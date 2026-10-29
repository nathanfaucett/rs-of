use http::StatusCode;
use reqwest::Client;
use serde_json::{Value, json};

use super::identity::{expect_status, user_token};

pub(crate) async fn run_checks(
    client: &Client,
    base: &str,
    template: &Value,
    admin: &str,
    app_admin: &str,
    unassigned: &str,
) {
    let register = format!("{base}/idp/oauth2/register");
    let applications = format!("{base}/idp/applications");
    let registrar = user_token(client, base, "registrar", "idp-services").await;
    let before = expect_status(
        client
            .get(&applications)
            .bearer_auth(admin)
            .send()
            .await
            .expect("list Applications"),
        StatusCode::OK,
    )
    .await;
    let mut request = template.clone();
    request["application"] =
        json!({"name":"Aicacia Password Manager", "uri":"password-manager.aicacia.com"});
    request["client_uri"] = "https://password-manager.aicacia.com/home".into();
    request["software_statement"] = "private-software-assertion".into();
    request["contacts"] = json!(["private-contact@example.test"]);
    for actor in [
        None,
        Some(unassigned),
        Some(app_admin),
        Some(registrar.as_str()),
    ] {
        let mut call = client.post(&register).json(&request);
        if let Some(token) = actor {
            call = call.bearer_auth(token);
        }
        expect_status(
            call.send().await.expect("denied onboarding"),
            if actor.is_none() {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::FORBIDDEN
            },
        )
        .await;
    }
    let mut chosen = request.clone();
    chosen["client_id"] = "password-manager-web".into();
    expect_status(
        client
            .post(&register)
            .bearer_auth(admin)
            .json(&chosen)
            .send()
            .await
            .expect("reject supplied identity"),
        StatusCode::BAD_REQUEST,
    )
    .await;
    for (field, value) in [
        ("client_type", json!("confidential")),
        ("token_endpoint_auth_method", json!("client_secret_basic")),
        ("client_secret", json!("caller-secret")),
    ] {
        let mut invalid = request.clone();
        invalid[field] = value;
        expect_status(
            client
                .post(&register)
                .bearer_auth(admin)
                .json(&invalid)
                .send()
                .await
                .expect("invalid public onboarding"),
            StatusCode::BAD_REQUEST,
        )
        .await;
    }
    let after = expect_status(
        client
            .get(&applications)
            .bearer_auth(admin)
            .send()
            .await
            .expect("denials leave Applications unchanged"),
        StatusCode::OK,
    )
    .await;
    assert_eq!(before, after);
    let created = expect_status(
        client
            .post(&register)
            .bearer_auth(admin)
            .json(&request)
            .send()
            .await
            .expect("authorized first-time onboarding"),
        StatusCode::OK,
    )
    .await;
    let id = created["client_id"]
        .as_str()
        .expect("server-issued identity");
    assert!(!id.is_empty());
    assert_ne!(id, "password-manager-web");
    assert!(created.get("client_secret").is_none());
    let apps = expect_status(
        client
            .get(&applications)
            .bearer_auth(admin)
            .send()
            .await
            .expect("read newly created Application"),
        StatusCode::OK,
    )
    .await;
    let app = apps
        .as_array()
        .expect("Application list")
        .iter()
        .find(|app| app["uri"] == request["application"]["uri"])
        .expect("canonical URI registered");
    assert_eq!(app["name"], "Aicacia Password Manager");
    let second = expect_status(
        client
            .post(&register)
            .bearer_auth(&registrar)
            .json(&request)
            .send()
            .await
            .expect("register-only permission reuses existing Application"),
        StatusCode::OK,
    )
    .await;
    assert_ne!(second["client_id"], created["client_id"]);
    let reused = expect_status(
        client
            .get(&applications)
            .bearer_auth(admin)
            .send()
            .await
            .expect("existing Application reused"),
        StatusCode::OK,
    )
    .await;
    assert_eq!(apps, reused);

    let metadata_url = format!("{base}/idp/oauth2/client/{id}");
    expect_status(
        client
            .get(&metadata_url)
            .send()
            .await
            .expect("consent metadata requires authentication"),
        StatusCode::UNAUTHORIZED,
    )
    .await;
    let metadata = expect_status(
        client
            .get(&metadata_url)
            .bearer_auth(unassigned)
            .send()
            .await
            .expect("cross-Application consent display"),
        StatusCode::OK,
    )
    .await;
    assert_eq!(
        metadata["application"]["uri"],
        "password-manager.aicacia.com"
    );
    assert_eq!(metadata["client_uri"], request["client_uri"]);
    assert_eq!(metadata["client_id"], id);
    let mut keys: Vec<_> = metadata
        .as_object()
        .expect("metadata object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "allowed_scopes",
            "application",
            "client_id",
            "client_name",
            "client_uri",
            "logo_uri",
            "policy_uri",
            "redirect_uris",
            "tos_uri"
        ]
    );
    for path in ["idp-evaluator", "missing-client"] {
        expect_status(
            client
                .get(format!("{base}/idp/oauth2/client/{path}"))
                .bearer_auth(unassigned)
                .send()
                .await
                .expect("no infrastructure or missing-client metadata"),
            StatusCode::NOT_FOUND,
        )
        .await;
    }
    let administration = format!("{register}/{id}");
    for actor in [admin, unassigned] {
        expect_status(
            client
                .get(&administration)
                .bearer_auth(actor)
                .send()
                .await
                .expect("consent display does not grant administrative read"),
            StatusCode::FORBIDDEN,
        )
        .await;
        expect_status(
            client
                .put(&administration)
                .bearer_auth(actor)
                .json(&created)
                .send()
                .await
                .expect("onboarding does not grant administrative update"),
            StatusCode::FORBIDDEN,
        )
        .await;
        expect_status(
            client
                .delete(&administration)
                .bearer_auth(actor)
                .send()
                .await
                .expect("onboarding does not grant administrative delete"),
            StatusCode::FORBIDDEN,
        )
        .await;
    }
    let redirect_uri = request["redirect_uris"][0]
        .as_str()
        .expect("requested redirect URI");
    let scope = request["allowed_scopes"]
        .as_array()
        .expect("requested scopes")
        .iter()
        .map(|value| value.as_str().expect("scope string"))
        .collect::<Vec<_>>()
        .join(" ");
    let consent = json!({"client_id":id, "redirect_uri":redirect_uri, "scope":scope});
    expect_status(
        client
            .post(format!("{base}/idp/oauth2/auth/approve-for-user"))
            .bearer_auth(unassigned)
            .json(&consent)
            .send()
            .await
            .expect("consent remains a separate User operation"),
        StatusCode::OK,
    )
    .await;

    let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    let authorization_url = format!("{base}/idp/oauth2/auth");
    let authorization = json!({
        "response_type":"code",
        "client_id":id,
        "redirect_uri":redirect_uri,
        "scope":scope,
        "state":"generated-client-pkce",
        "code_challenge":"E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
        "code_challenge_method":"S256",
    });
    let mut original_id = authorization.clone();
    original_id["client_id"] = "password-manager-web".into();
    let denied = expect_status(
        client
            .post(&authorization_url)
            .bearer_auth(unassigned)
            .json(&original_id)
            .send()
            .await
            .expect("hardcoded Client ID cannot authorize"),
        StatusCode::BAD_REQUEST,
    )
    .await;
    assert_eq!(denied["error"], "invalid_client");
    let authorized = expect_status(
        client
            .post(&authorization_url)
            .bearer_auth(unassigned)
            .json(&authorization)
            .send()
            .await
            .expect("User authorizes the generated Client with PKCE"),
        StatusCode::OK,
    )
    .await;
    assert_eq!(authorized["state"], "generated-client-pkce");
    let code = authorized["code"].as_str().expect("authorization code");
    assert!(!code.is_empty());
    let token_url = format!("{base}/idp/oauth2/token");
    for (client_id, code_verifier, error) in [
        ("password-manager-web", verifier, "invalid_client"),
        (id, "wrong-verifier", "invalid_grant"),
    ] {
        let denied = expect_status(
            client
                .post(&token_url)
                .form(&[
                    ("grant_type", "authorization_code"),
                    ("client_id", client_id),
                    ("code", code),
                    ("redirect_uri", redirect_uri),
                    ("code_verifier", code_verifier),
                ])
                .send()
                .await
                .expect("Client ID and PKCE mismatch cannot redeem code"),
            StatusCode::BAD_REQUEST,
        )
        .await;
        assert_eq!(denied["error"], error);
    }
    let tokens = expect_status(
        client
            .post(&token_url)
            .form(&[
                ("grant_type", "authorization_code"),
                ("client_id", id),
                ("code", code),
                ("redirect_uri", redirect_uri),
                ("code_verifier", verifier),
            ])
            .send()
            .await
            .expect("public Client redeems PKCE code without a secret"),
        StatusCode::OK,
    )
    .await;
    let access_token = tokens["access_token"]
        .as_str()
        .expect("issued access token");
    assert!(!access_token.is_empty());
    assert!(tokens.get("client_secret").is_none());
    let sdk_metadata = expect_status(
        client
            .get(&metadata_url)
            .bearer_auth(access_token)
            .send()
            .await
            .expect("SDK reads consent metadata using the generated Client's access token"),
        StatusCode::OK,
    )
    .await;
    assert_eq!(sdk_metadata, metadata);
}
