use std::{
    fs,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use axum::{
    Json, Router,
    extract::{Request, State},
    http::StatusCode,
    routing::{delete, get, post},
};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use crate::setup_reset;

struct Peer {
    issuer: Mutex<String>,
    deny_device: AtomicBool,
    exchanges: Mutex<Vec<String>>,
    deletions: Mutex<Vec<String>>,
}

async fn metadata(State(peer): State<Arc<Peer>>) -> Json<Value> {
    Json(json!({"issuer":peer.issuer.lock().expect("fixture issuer").clone()}))
}

async fn exchange(State(peer): State<Arc<Peer>>, request: Request) -> Json<Value> {
    let bytes = axum::body::to_bytes(request.into_body(), 4096)
        .await
        .expect("bounded fixture exchange");
    peer.exchanges
        .lock()
        .expect("exchange records")
        .push(String::from_utf8(bytes.to_vec()).expect("form input"));
    Json(json!({"access_token":"exchanged-peer-user", "token_type":"Bearer"}))
}

async fn revoke(State(peer): State<Arc<Peer>>, request: Request) -> StatusCode {
    assert_eq!(
        request.headers()["Authorization"],
        "Bearer exchanged-peer-user"
    );
    let path = request.uri().path().to_owned();
    let management = path.starts_with("/management");
    let denied = management && peer.deny_device.load(Ordering::SeqCst);
    peer.deletions.lock().expect("deletion records").push(path);
    if denied {
        StatusCode::FORBIDDEN
    } else if management {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::OK
    }
}

#[tokio::test]
async fn peer_reset_revocation_pins_issuer_exchanges_each_audience_and_acknowledges_only_both_owners()
 {
    let root =
        std::env::temp_dir().join(format!("native-peer-revocation-{}", uuid::Uuid::new_v4()));
    let setup = crate::native_host::prepare(&root.join("peer")).expect("isolated TLS only");
    let tls = setup.tls().clone();
    let address = setup.local_addr();
    let peer_api = format!("https://{address}/idp");
    let certificate = reqwest::Certificate::from_pem(&fs::read(tls.ca_path()).expect("fixture CA"))
        .expect("certificate");
    let client = super::client_builder()
        .use_rustls_tls()
        .tls_built_in_root_certs(false)
        .add_root_certificate(certificate)
        .no_proxy()
        .build()
        .expect("fixture-only trust");
    let state = Arc::new(Peer {
        issuer: Mutex::new("https://wrong-installation.test/idp".into()),
        deny_device: AtomicBool::new(true),
        exchanges: Mutex::new(Vec::new()),
        deletions: Mutex::new(Vec::new()),
    });
    let router = Router::new()
        .route("/idp/.well-known/openid-configuration", get(metadata))
        .route("/idp/oauth2/token", post(exchange))
        .route("/idp/replica-signers/{id}", delete(revoke))
        .route("/management/devices/{id}", delete(revoke))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("isolated peer listener");
    let cancellation = CancellationToken::new();
    let shutdown = cancellation.clone();
    let server = tokio::spawn(async move {
        axum::serve(tls.listener(listener), router)
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await
    });
    let data = root.join("reset-profile");
    fs::create_dir(&data).expect("isolated reset profile");
    let member = idp_model::model::Id::now_v7();
    setup_reset::publish(&data, &format!("reset-revocation-{member}.json"), &json!({"version":1, "issuer":"https://installation.test/idp", "member_id":member, "status":"pending"})).expect("retained public target");
    assert!(
        super::revoke(
            &data,
            "https://local.test/idp",
            &peer_api,
            "user-session",
            member,
            &client
        )
        .await
        .is_err()
    );
    assert!(
        state.exchanges.lock().expect("records").is_empty(),
        "wrong issuer must receive no User token"
    );
    *state.issuer.lock().expect("issuer") = "https://installation.test/idp".into();
    assert!(
        super::revoke(
            &data,
            "https://local.test/idp",
            &peer_api,
            "user-session",
            member,
            &client
        )
        .await
        .is_err()
    );
    assert_eq!(
        setup_reset::revocations(&data).expect("retry target")[0]["status"],
        "pending"
    );
    assert_eq!(
        state.deletions.lock().expect("deletions").as_slice(),
        [
            format!("/idp/replica-signers/{member}"),
            format!("/management/devices/{member}")
        ]
    );
    let exchanges = state.exchanges.lock().expect("exchange records").clone();
    assert_eq!(exchanges.len(), 2);
    for (body, resource) in exchanges.iter().zip([
        peer_api.as_str(),
        management_server::MANAGEMENT_APPLICATION_URI,
    ]) {
        let parsed = reqwest::Url::parse(&format!("https://fixture.test/?{body}"))
            .expect("parse submitted form");
        let form = parsed
            .query_pairs()
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(
            form.get("subject_token").expect("User session").as_ref(),
            "user-session"
        );
        assert_eq!(
            form.get("resource").expect("exact owner audience").as_ref(),
            resource
        );
    }
    state.deny_device.store(false, Ordering::SeqCst);
    super::revoke(
        &data,
        "https://local.test/idp",
        &peer_api,
        "user-session",
        member,
        &client,
    )
    .await
    .expect("both owner acknowledgements");
    assert_eq!(
        setup_reset::revocations(&data).expect("acknowledged target")[0]["status"],
        "acknowledged"
    );
    cancellation.cancel();
    server
        .await
        .expect("join fixture server")
        .expect("close listener");
    drop(setup);
    fs::remove_dir_all(root).expect("isolated cleanup");
}
