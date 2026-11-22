#[cfg(all(test, feature = "ofdb"))]
use std::sync::Arc;

use iroh::{SecretKey, endpoint::presets::Minimal};
use iroh_chain::{EndpointIdStore, Server};
use ofdb_sql::Database;
use tokio_util::sync::CancellationToken;

use crate::{security_policy::SecurityContext, security_protocol::SecurityProtocol};

#[tokio::test]
async fn owner_rejects_unreviewed_public_schema() {
    let idp = Arc::new(Database::in_memory());
    db::ofdb::up(&idp.client()).await.expect("identity schema");
    let devices = Arc::new(Database::in_memory());
    crate::ofdb::schema::up(&devices)
        .await
        .expect("Device schema");
    let server =
        Server::bind_with_secret_key(Minimal, SecretKey::generate(), EndpointIdStore::default())
            .await
            .expect("local endpoint");
    let protocol = SecurityProtocol::new(
        server.clone(),
        idp.clone(),
        devices.clone(),
        idp,
        "https://installation.example".into(),
        std::env::temp_dir(),
        CancellationToken::new(),
    );
    protocol
        .validate_local_schema(SecurityContext::Device)
        .await
        .expect("reviewed owner schemas");
    devices
        .client()
        .translate_and_execute(
            "ALTER TABLE devices ADD COLUMN unreviewed TEXT",
            &ofdb_sql::SqlTranslator,
        )
        .await
        .expect("unreviewed column");
    let error = protocol
        .validate_local_schema(SecurityContext::Device)
        .await
        .expect_err("unknown fields must not synchronize");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    server.endpoint().close().await;
}
