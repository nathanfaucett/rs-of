use of_model::device::device::Device;
use serde_json::json;

#[test]
fn device_persistence_and_public_info_keep_their_wire_formats() {
    let stored = json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "owner_subject": "owner",
        "name": "device",
        "public_key": "key",
        "address": "address",
        "state": 1,
        "created_at": 1,
        "updated_at": 2,
        "revoked_at": null
    });
    for (state, name) in [(0, "pending"), (1, "approved"), (2, "revoked")] {
        let mut value = stored.clone();
        value["state"] = json!(state);
        let device: Device = serde_json::from_value(value.clone()).expect("known Device state");
        assert_eq!(serde_json::to_value(&device).expect("stored state"), value);
        assert_eq!(
            serde_json::to_value(of_model::device::contract::device_info::DeviceInfo::from(
                device
            ))
            .expect("public state")["state"],
            name
        );
    }
    let device: Device = serde_json::from_value(stored.clone()).expect("stored device");
    assert_eq!(
        device.state,
        of_model::device::contract::device_state::DeviceState::Approved
    );
    assert_eq!(
        serde_json::to_value(&device).expect("stored format"),
        stored
    );
    let info = of_model::device::contract::device_info::DeviceInfo::from(device);
    assert_eq!(
        serde_json::to_value(info).expect("public device info"),
        json!({
            "id": "00000000-0000-0000-0000-000000000001",
            "name": "device",
            "publicKey": "key",
            "address": "address",
            "state": "approved",
            "createdAt": 1,
            "updatedAt": 2,
            "revokedAt": null
        })
    );
    let mut invalid = stored;
    invalid["state"] = json!(3);
    assert!(serde_json::from_value::<Device>(invalid).is_err());
}

#[test]
fn device_pairing_signed_payload_keeps_its_length_delimited_bytes() {
    let id = of_model::id::Id::parse_str("00000000-0000-0000-0000-000000000001")
        .expect("fixed Device identity");
    assert_eq!(
        of_model::device::contract::device_pairing::device_pairing_approval_payload(
            id, "é\n", "key", "local"
        ),
        "idp-device-pairing-approval-v1\n00000000-0000-0000-0000-000000000001\n3\né\n\n3\nkey\n5\nlocal"
    );
}

#[cfg(feature = "utoipa")]
#[test]
fn device_contracts_export_api_schemas_from_management() {
    fn schema<T: utoipa::ToSchema>() {}

    schema::<of_model::device::contract::device_state::DeviceState>();
    schema::<of_model::device::contract::device_info::DeviceInfo>();
    schema::<of_model::device::contract::device_enrollment::DeviceEnrollment>();
    schema::<of_model::device::contract::device_enrollment_request::DeviceEnrollmentRequest>();
    schema::<of_model::device::contract::device_approval_request::DeviceApprovalRequest>();
    schema::<of_model::device::contract::device_pairing::DevicePairingRequest>();
    schema::<of_model::device::contract::device_pairing::DevicePairingApprovalRequest>();
    schema::<of_model::device::contract::device_pairing::DevicePairingApprovalPayload>();
    schema::<of_model::device::contract::device_endpoint_identity::DeviceEndpointIdentity>();
    schema::<of_model::device::contract::approved_device_endpoints::ApprovedDeviceEndpoints>();
    schema::<of_model::device::contract::trusted_device::TrustedDevice>();
    schema::<of_model::device::contract::update_device_request::UpdateDeviceRequest>();
    schema::<of_model::device::contract::update_device_request::DeviceTransferRequest>();
    schema::<of_model::device::contract::device_self_revocation::DeviceSelfRevocationRequest>();
}

#[cfg(feature = "ofdb")]
#[tokio::test]
async fn standard_pairing_requires_endpoint_proof_and_allows_revocation() {
    use std::sync::Arc;

    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use device_service::{
        DeviceEnrollmentRequest, DeviceEnrollmentService, DevicePairingApprovalRequest,
        DevicePairingRequest, DeviceRepo, DeviceState, ofdb::DbDeviceRepo,
    };
    use iroh::SecretKey;
    use of_model::id::Id;
    use ofdb_sql::Database;

    let database = Database::in_memory();
    device_service::ofdb::up(&database)
        .await
        .expect("Device owner schema");
    let repo = Arc::new(DbDeviceRepo::new(database.client()));
    let accepting = SecretKey::generate();
    let owner = Id::now_v7().to_string();
    let enrollment = DeviceEnrollmentService::new(repo.clone());
    let initial = enrollment
        .enroll(
            owner.clone(),
            DeviceEnrollmentRequest {
                name: "First Device".into(),
                public_key: accepting.public().to_string(),
                address: "https://first.test".into(),
            },
        )
        .await
        .expect("first Device enrollment");
    assert_eq!(initial.state, DeviceState::Approved);

    let peer = SecretKey::generate();
    let endpoint = peer.public().to_string();
    let pending = enrollment
        .request_pairing(DevicePairingRequest {
            name: "Second Device".into(),
            public_key: endpoint.clone(),
            address: "https://second.test".into(),
            accepting_public_key: accepting.public().to_string(),
        })
        .await
        .expect("pairing request in the existing Installation");
    assert_eq!(pending.state, DeviceState::Pending);
    let payload = enrollment
        .pairing_approval_payload(pending.id)
        .await
        .expect("exact approval payload");
    let proof = |key: &SecretKey| DevicePairingApprovalRequest {
        signature: URL_SAFE_NO_PAD.encode(key.sign(payload.as_bytes()).to_bytes()),
    };
    assert!(
        enrollment
            .approve_pairing(pending.id, proof(&accepting))
            .await
            .is_err()
    );
    assert!(
        repo.find_approved_by_public_key(&endpoint)
            .await
            .expect("Device lookup")
            .is_none()
    );
    assert_eq!(
        enrollment
            .approve_pairing(pending.id, proof(&peer))
            .await
            .expect("new endpoint proof")
            .state,
        DeviceState::Approved
    );
    enrollment
        .revoke(&owner, pending.id, &accepting.public().to_string())
        .await
        .expect("Device revocation");
    assert!(
        repo.find_approved_by_public_key(&endpoint)
            .await
            .expect("Device lookup")
            .is_none()
    );
    assert!(
        enrollment
            .approve_pairing(pending.id, proof(&accepting))
            .await
            .is_err()
    );
}
