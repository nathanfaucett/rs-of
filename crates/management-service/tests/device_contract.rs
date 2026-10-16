use management_service::{Device, DeviceInfo, DeviceState};
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
            serde_json::to_value(DeviceInfo::from(device)).expect("public state")["state"],
            name
        );
    }
    let device: Device = serde_json::from_value(stored.clone()).expect("stored device");
    assert_eq!(device.state, DeviceState::Approved);
    assert_eq!(
        serde_json::to_value(&device).expect("stored format"),
        stored
    );
    let info = DeviceInfo::from(device);
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
    let id = idp_model::model::Id::parse_str("00000000-0000-0000-0000-000000000001")
        .expect("fixed Device identity");
    assert_eq!(
        management_service::device_pairing_approval_payload(id, "é\n", "key", "local"),
        "idp-device-pairing-approval-v1\n00000000-0000-0000-0000-000000000001\n3\né\n\n3\nkey\n5\nlocal"
    );
}

#[cfg(feature = "utoipa")]
#[test]
fn device_contracts_export_api_schemas_from_management() {
    fn schema<T: utoipa::ToSchema>() {}

    schema::<management_service::DeviceState>();
    schema::<management_service::DeviceInfo>();
    schema::<management_service::DeviceEnrollment>();
    schema::<management_service::DeviceEnrollmentRequest>();
    schema::<management_service::DeviceApprovalRequest>();
    schema::<management_service::DevicePairingRequest>();
    schema::<management_service::DevicePairingApprovalRequest>();
    schema::<management_service::DevicePairingApprovalPayload>();
    schema::<management_service::PairingAcceptance>();
    schema::<management_service::DeviceEndpointIdentity>();
    schema::<management_service::ApprovedDeviceEndpoints>();
    schema::<management_service::TrustedDevice>();
    schema::<management_service::UpdateDeviceRequest>();
    schema::<management_service::DeviceTransferRequest>();
    schema::<management_service::DeviceSelfRevocationRequest>();
}
