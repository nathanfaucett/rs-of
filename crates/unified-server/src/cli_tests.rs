use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use super::{SetupTarget, submit};
use crate::{SetupHost, UnifiedConfig};

#[tokio::test]
async fn exact_retry_observes_retired_owner_and_submits_only_incomplete_owners() {
    for conflict in [
        "none",
        "retirement-race",
        "nonce",
        "node",
        "issuer",
        "purpose",
        "administrator",
        "idp",
        "device",
        "storage",
        "peer",
        "owner",
        "not-ready",
        "status-owner",
        "missing-public",
    ] {
        let root = std::env::temp_dir().join(format!(
            "cli-owner-retry-{}",
            idp_model::model::Id::now_v7()
        ));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let address = listener.local_addr().expect("bound address");
        let config = UnifiedConfig::local(&root, address);
        let host = SetupHost::prepare(config.clone()).expect("TLS and owners");
        let mut public_operation = json!({
            "purpose":"create", "operation_nonce":"0123456789abcdef0123456789abcdef",
            "administrator":"admin", "node":"My Node", "issuer":config.idp.api_public_uri,
            "idp":config.idp.api_public_uri, "device":config.device.api_public_uri,
            "storage":config.storage.api_public_uri, "peer":null
        });
        match conflict {
            "nonce" => {
                public_operation["operation_nonce"] = json!("ffffffffffffffffffffffffffffffff")
            }
            "node" => public_operation["node"] = json!("Other Node"),
            "issuer" => public_operation["issuer"] = json!("https://other.example/idp"),
            "purpose" => public_operation["purpose"] = json!("join"),
            "administrator" => public_operation["administrator"] = json!("other"),
            "idp" | "device" | "storage" => {
                public_operation[conflict] = json!("https://other.example/owner")
            }
            "peer" => public_operation["peer"] = json!("https://other.example/device"),
            _ => (),
        }
        let retired = root.join("retired-admission");
        let admission = api::SetupAdmission::open(&retired).expect("admission");
        let code = api::SetupAdmission::protected_code(&retired)
            .expect("code")
            .expect("live code");
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Setup {code}").parse().expect("header"),
        );
        let exact = api::IndependentCreateIntent {
            purpose: "create".into(),
            operation_nonce: "0123456789abcdef0123456789abcdef".into(),
            administrator: "admin".into(),
            node: "My Node".into(),
            issuer: config.idp.api_public_uri.clone(),
            idp: config.idp.api_public_uri.clone(),
            device: config.device.api_public_uri.clone(),
            storage: config.storage.api_public_uri.clone(),
            peer: None,
        }
        .canonical()
        .expect("canonical operation");
        admission.bind(&headers, &exact).expect("bind");
        if conflict != "retirement-race" {
            admission.retire(&headers, &exact).expect("retire");
        }
        drop(admission);
        let posts = Arc::new(Mutex::new(Vec::<(String, Value)>::new()));
        let mut router = Router::new()
            .route("/idp/setup/owner/status", get(move |headers: HeaderMap| async move {
                assert!(!headers.contains_key("Authorization"), "completion observation grants no authority");
                Json(json!({"stage":"installation", "state":if conflict == "not-ready" {"running"} else {"ready"},
                    "ready":conflict != "not-ready", "owners":[if conflict == "status-owner" {"device"} else {"idp"}]}))
            }))
            .route("/idp/setup/owner/public", get(move |headers: HeaderMap| async move {
                assert!(!headers.contains_key("Authorization"), "public exactness observation grants no authority");
                if conflict == "missing-public" { return (StatusCode::CONFLICT, Json(json!({}))); }
                (StatusCode::OK, Json(json!({"owner":if conflict == "owner" {"device"} else {"idp"},
                    "operation":public_operation, "clients":[], "facts":{}, "registered":[]})))
            }))
            .route("/management/health", get(|| async { StatusCode::OK }));
        let mut paths = Vec::new();
        for owner in ["idp", "device", "storage"] {
            let path = if owner == "idp" {
                retired.clone()
            } else {
                root.join(format!("{owner}-admission"))
            };
            if owner != "idp" {
                drop(api::SetupAdmission::open(&path).expect("live owner admission"));
            }
            let expected = api::SetupAdmission::protected_code(&path).expect("protected state");
            paths.push(path.join("state"));
            let posts = posts.clone();
            let exact = exact.clone();
            router = router.route(
                &format!("/{owner}/setup/owner/create"),
                post(
                    move |headers: HeaderMap, Json(body): Json<Value>| async move {
                        assert_eq!(
                            headers.get("Authorization").and_then(|v| v.to_str().ok()),
                            expected
                                .as_ref()
                                .map(|code| format!("Setup {code}"))
                                .as_deref()
                        );
                        posts.lock().expect("posts").push((owner.into(), body));
                        if owner == "idp" && conflict == "retirement-race" {
                            api::SetupAdmission::open(&path)
                                .expect("retiring owner")
                                .retire(&headers, &exact)
                                .expect("retire during request");
                            return StatusCode::UNAUTHORIZED;
                        }
                        StatusCode::ACCEPTED
                    },
                ),
            );
        }
        let stop = CancellationToken::new();
        let cancellation = stop.clone();
        let tls = host.tls().clone();
        let server = tokio::spawn(async move {
            axum::serve(tls.listener(listener), router.into_make_service())
                .with_graceful_shutdown(cancellation.cancelled_owned())
                .await
                .expect("HTTPS fixture");
        });
        let result = submit(
            SetupTarget {
                idp: config.idp.api_public_uri.clone(),
                management: config.management.api_public_uri.clone(),
                device: config.device.api_public_uri.clone(),
                storage: config.storage.api_public_uri.clone(),
                operation_nonce: "0123456789abcdef0123456789abcdef".into(),
                ca_path: host.tls().ca_path(),
                idp_code_file: paths[0].clone(),
                device_code_file: paths[1].clone(),
                storage_code_file: paths[2].clone(),
            },
            "admin".into(),
            "My Node".into(),
            Some("secret password".into()),
            None,
        )
        .await;
        let success = matches!(conflict, "none" | "retirement-race");
        assert_eq!(result.is_ok(), success, "{conflict}: {result:?}");
        {
            let submitted = posts.lock().expect("submitted requests");
            if success {
                let expected = if conflict == "retirement-race" {
                    vec!["idp", "device", "storage"]
                } else {
                    vec!["device", "storage"]
                };
                assert_eq!(
                    submitted
                        .iter()
                        .map(|(owner, _)| owner.as_str())
                        .collect::<Vec<_>>(),
                    expected
                );
                for (owner, body) in submitted.iter() {
                    assert_eq!(body.get("password").is_some(), owner == "idp");
                    assert_eq!(body["operation_nonce"], "0123456789abcdef0123456789abcdef");
                }
            } else {
                assert!(
                    submitted.is_empty(),
                    "conflicting completion must stop later submissions"
                );
            }
        }
        assert!(
            api::SetupAdmission::protected_code(&retired)
                .expect("retired state")
                .is_none()
        );
        stop.cancel();
        server.await.expect("fixture stopped");
        drop(host);
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
