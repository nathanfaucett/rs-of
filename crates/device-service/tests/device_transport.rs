#![cfg(feature = "std")]

use std::{sync::Arc, time::Duration};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use device_service::DeviceIdentity;
use device_service::PairingAcceptanceController;
use device_service::PairingAcceptanceControllerSlot;
use device_service::TimedPairingAcceptanceController;
use device_service::device_identity_from_server;
use iroh::{EndpointAddr, SecretKey, Signature, endpoint::presets};
use iroh_chain::{EndpointIdStore, Server};

#[tokio::test]
async fn shared_endpoint_identity_signs_without_an_idp_signer_or_database() {
    let secret = SecretKey::generate();
    let server =
        Server::bind_with_secret_key(presets::Minimal, secret.clone(), EndpointIdStore::default())
            .await
            .expect("bind shared endpoint");
    assert!(device_identity_from_server(&server, SecretKey::generate()).is_err());
    assert!(DeviceIdentity::new(server.endpoint().clone(), SecretKey::generate()).is_err());
    let identity = device_identity_from_server(&server, secret).expect("exact endpoint key");
    assert_eq!(identity.endpoint_id(), server.endpoint().id());
    assert_eq!(identity.endpoint().id(), server.endpoint().id());
    let address: EndpointAddr = serde_json::from_str(
        &identity
            .endpoint_address()
            .expect("public endpoint address"),
    )
    .expect("transport endpoint address");
    assert_eq!(address.id, server.endpoint().id());

    let payload = b"of.equal-node-join-approval.v1:exact-owner-and-request";
    let signature = identity.sign(payload);
    identity
        .endpoint_id()
        .verify(payload, &signature)
        .expect("endpoint signature");
    assert!(
        identity
            .endpoint_id()
            .verify(b"different owner or request", &signature)
            .is_err()
    );

    let revocation = identity.self_revocation_request();
    assert_eq!(revocation.public_key, identity.endpoint_id().to_string());
    let bytes = URL_SAFE_NO_PAD
        .decode(revocation.signature)
        .expect("encoded signature");
    let signature = Signature::try_from(bytes.as_slice()).expect("endpoint signature bytes");
    let expected = format!(
        "idp-device-self-revocation-v1\n64\n{}",
        identity.endpoint_id()
    );
    identity
        .endpoint_id()
        .verify(expected.as_bytes(), &signature)
        .expect("existing revocation wire bytes");
    assert!(
        SecretKey::generate()
            .public()
            .verify(expected.as_bytes(), &signature)
            .is_err()
    );
    server.endpoint().close().await;
}

#[tokio::test]
async fn pairing_control_requires_a_bound_controller_and_uses_the_shared_endpoint() {
    let server = Server::bind_with_secret_key(
        presets::Minimal,
        SecretKey::generate(),
        EndpointIdStore::default(),
    )
    .await
    .expect("bind pairing endpoint");
    server.set_pairing_enabled(false);
    let slot = PairingAcceptanceControllerSlot::new();
    assert!(slot.pairing_accepting().is_err());
    assert!(slot.set_pairing_accepting(true).is_err());
    slot.bind(Arc::new(TimedPairingAcceptanceController::new(
        server.clone(),
        Duration::from_secs(1),
    )))
    .expect("bind Management pairing controller");
    assert!(!slot.pairing_accepting().expect("initial pairing state"));
    slot.set_pairing_accepting(true).expect("enable pairing");
    assert!(server.pairing_enabled());
    slot.set_pairing_accepting(false).expect("disable pairing");
    assert!(!server.pairing_enabled());
    assert!(!slot.pairing_accepting().expect("disabled pairing state"));
    server.endpoint().close().await;
}
