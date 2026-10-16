use serde_json::{Value, json};
use utoipa::OpenApi;

use super::{CreateDoc, CreateResult, LoginClient};

#[test]
fn join_contract_requires_setup_auth_and_server_discovered_issuer() {
    let doc = document();
    let join = &doc["paths"]["/idp/setup/join"]["post"];
    assert_eq!(join["security"], json!([{ "setup_authorization": [] }]));
    assert_eq!(
        join["requestBody"]["content"]["application/json"]["schema"]["$ref"],
        "#/components/schemas/JoinRequest"
    );
    assert!(join["responses"]["202"].is_object());
    assert!(join["responses"].get("200").is_none());
    let fields = doc["components"]["schemas"]["JoinRequest"]["properties"]
        .as_object()
        .expect("Join fields");
    assert_eq!(
        fields.keys().map(String::as_str).collect::<Vec<_>>(),
        ["node", "operation_nonce", "peer"]
    );
    assert_eq!(
        doc["components"]["schemas"]["JoinRequest"]["additionalProperties"],
        false
    );
}

#[test]
fn setup_openapi_separates_setup_capability_from_completed_password_retry() {
    let doc = document();
    let scheme = &doc["components"]["securitySchemes"]["setup_authorization"];
    assert_eq!(scheme["type"], "apiKey");
    assert_eq!(scheme["in"], "header");
    assert_eq!(scheme["name"], "Authorization");
    assert!(
        scheme["description"]
            .as_str()
            .expect("setup header usage")
            .contains("Setup <code>")
    );
    assert_eq!(
        doc["components"]["securitySchemes"]
            .as_object()
            .expect("setup schemes")
            .len(),
        1
    );
    for path in ["/idp/setup/cancel", "/idp/setup/admission"] {
        assert_eq!(
            doc["paths"][path]["post"]["security"],
            json!([{ "setup_authorization": [] }])
        );
    }
    assert_eq!(
        doc["paths"]["/idp/setup/create"]["post"]["security"],
        json!([{}, { "setup_authorization": [] }])
    );
    assert_eq!(
        doc["paths"]["/idp/setup/status"]["get"]["security"],
        json!([])
    );
    assert!(doc.get("security").is_none());
    assert_eq!(doc["servers"][0]["url"], "https://localhost");
}

#[test]
fn setup_openapi_keeps_password_write_only_and_status_nonsecret() {
    let doc = document();
    let schemas = &doc["components"]["schemas"];
    assert_eq!(
        schemas["CreateRequest"]["properties"]["password"]["writeOnly"],
        true
    );
    assert_eq!(
        schemas["CreateRequest"]["properties"]["password"]["format"],
        "password"
    );
    for (schema, expected) in [
        (
            "CreateRequest",
            &["administrator", "node", "operation_nonce", "password"][..],
        ),
        (
            "CreateIntent",
            &["administrator", "node", "operation_nonce"][..],
        ),
        (
            "CreateResult",
            &["login_client", "owners", "ready", "stage", "state"][..],
        ),
        ("LoginClient", &["client_id", "scopes"][..]),
    ] {
        assert_eq!(
            schemas[schema]["properties"]
                .as_object()
                .expect("wire properties")
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            expected
        );
    }
    for schema in ["CreateRequest", "CreateIntent"] {
        assert_eq!(schemas[schema]["additionalProperties"], false);
        let mut required = schemas[schema]["required"]
            .as_array()
            .expect("required intent fields")
            .iter()
            .map(|field| field.as_str().expect("field name"))
            .collect::<Vec<_>>();
        required.sort_unstable();
        let expected = if schema == "CreateRequest" {
            vec!["administrator", "node", "operation_nonce", "password"]
        } else {
            vec!["administrator", "node", "operation_nonce"]
        };
        assert_eq!(required, expected);
    }
    let mut result = CreateResult::unfinished("creating");
    assert_eq!(
        serde_json::to_value(&result).expect("accepted result"),
        json!({
            "stage": "installation", "state": "creating", "ready": false,
            "owners": ["idp", "management", "storage"]
        })
    );
    result.state = "ready".into();
    result.ready = true;
    result.login_client = Some(LoginClient {
        client_id: "public-login-client".into(),
        scopes: vec!["openid".into()],
    });
    assert_eq!(
        serde_json::to_value(result).expect("ready result"),
        json!({
            "stage": "installation", "state": "ready", "ready": true,
            "owners": ["idp", "management", "storage"],
            "login_client": { "client_id": "public-login-client", "scopes": ["openid"] }
        })
    );
}

#[test]
fn retained_intent_openapi_requires_setup_header_and_has_no_request_body() {
    let doc = document();
    let path = &doc["paths"]["/idp/setup/intent"];
    assert_eq!(path.as_object().expect("intent path").len(), 1);
    let operation = &path["post"];
    assert_eq!(
        operation["security"],
        json!([{ "setup_authorization": [] }])
    );
    assert!(operation.get("requestBody").is_none());
    assert_eq!(
        operation["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
        "#/components/schemas/SetupIntent"
    );
    for code in ["204", "401", "403", "503"] {
        assert!(operation["responses"][code].get("content").is_none());
    }
}

#[test]
#[ignore = "explicit generation workflow supplies OF_SETUP_OPENAPI_OUT; no host or provider changes"]
fn export_setup_openapi_for_client_generation() {
    let path = std::env::var_os("OF_SETUP_OPENAPI_OUT").expect("explicit setup schema destination");
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&document()).expect("setup schema JSON"),
    )
    .expect("write requested public setup schema");
}

fn document() -> Value {
    serde_json::to_value(CreateDoc::openapi()).expect("serializable setup OpenAPI contract")
}

#[test]
fn setup_openapi_describes_async_create_status_cancel_and_admission() {
    let doc = document();
    let paths = doc["paths"].as_object().expect("setup paths");
    assert_eq!(paths.len(), 6);
    for (path, method, operation, codes) in [
        (
            "/idp/setup/create",
            "post",
            "create",
            &[
                "200", "202", "400", "401", "403", "409", "413", "415", "422", "503",
            ][..],
        ),
        (
            "/idp/setup/join",
            "post",
            "join",
            &[
                "202", "400", "401", "403", "409", "413", "415", "422", "503",
            ][..],
        ),
        (
            "/idp/setup/status",
            "get",
            "status",
            &["200", "403", "503"][..],
        ),
        (
            "/idp/setup/cancel",
            "post",
            "cancel",
            &[
                "204", "400", "401", "403", "409", "413", "415", "422", "503",
            ][..],
        ),
        (
            "/idp/setup/intent",
            "post",
            "intent",
            &["200", "204", "401", "403", "503"][..],
        ),
        (
            "/idp/setup/admission",
            "post",
            "admission",
            &["204", "401", "403", "503"][..],
        ),
    ] {
        assert_eq!(doc["paths"][path][method]["operationId"], operation);
        let responses = doc["paths"][path][method]["responses"]
            .as_object()
            .expect("documented operation responses");
        assert_eq!(
            responses.keys().map(String::as_str).collect::<Vec<_>>(),
            codes
        );
    }
    for (path, method, codes) in [
        ("/idp/setup/create", "post", &["200", "202"][..]),
        ("/idp/setup/status", "get", &["200", "503"][..]),
    ] {
        for code in codes {
            assert_eq!(
                doc["paths"][path][method]["responses"][code]["content"]["application/json"]["schema"]
                    ["$ref"],
                "#/components/schemas/CreateResult"
            );
        }
    }
    for path in ["/idp/setup/cancel", "/idp/setup/admission"] {
        assert!(
            doc["paths"][path]["post"]["responses"]["204"]
                .get("content")
                .is_none()
        );
    }
    assert!(
        doc["paths"]["/idp/setup/create"]["post"]["responses"]["503"]
            .get("content")
            .is_none()
    );
    for (path, schema) in [
        ("/idp/setup/create", "CreateRequest"),
        ("/idp/setup/cancel", "SetupIntent"),
    ] {
        assert_eq!(doc["paths"][path]["post"]["requestBody"]["required"], true);
        assert_eq!(
            doc["paths"][path]["post"]["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            format!("#/components/schemas/{schema}")
        );
    }
}
