#![cfg(feature = "std")]

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use device_service::{DeviceIdentity, device_identity_from_server};
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

    let payload = b"device transport identity proof";
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
