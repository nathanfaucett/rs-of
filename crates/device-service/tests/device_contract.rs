use device_service::Device;
use device_service::DeviceInfo;
use device_service::DeviceState;
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
        device_service::device_pairing_approval_payload(id, "é\n", "key", "local"),
        "idp-device-pairing-approval-v1\n00000000-0000-0000-0000-000000000001\n3\né\n\n3\nkey\n5\nlocal"
    );
}

#[cfg(feature = "utoipa")]
#[test]
fn device_contracts_export_api_schemas_from_management() {
    fn schema<T: utoipa::ToSchema>() {}

    schema::<device_service::DeviceState>();
    schema::<device_service::DeviceInfo>();
    schema::<device_service::DeviceEnrollment>();
    schema::<device_service::DeviceEnrollmentRequest>();
    schema::<device_service::DeviceApprovalRequest>();
    schema::<device_service::DevicePairingRequest>();
    schema::<device_service::DevicePairingApprovalRequest>();
    schema::<device_service::DevicePairingApprovalPayload>();
    schema::<device_service::PairingAcceptance>();
    schema::<device_service::DeviceEndpointIdentity>();
    schema::<device_service::ApprovedDeviceEndpoints>();
    schema::<device_service::TrustedDevice>();
    schema::<device_service::UpdateDeviceRequest>();
    schema::<device_service::DeviceTransferRequest>();
    schema::<device_service::DeviceSelfRevocationRequest>();
}

#[cfg(feature = "replica")]
#[tokio::test]
async fn approved_enrollment_preserves_exact_identity_and_denies_replacement_or_revocation() {
    use device_service::DeviceRepo;
    use device_service::replica::DbDeviceRepo;
    use idp_model::model::Id;
    use std::sync::Arc;

    let engine = Arc::new(db::Database::in_memory());
    device_service::replica::up(&engine)
        .await
        .expect("Management schema");
    let repo = DbDeviceRepo::new(engine.clone().client());
    let id = Id::now_v7();
    let owner = Id::now_v7();
    let endpoint = iroh::SecretKey::generate().public().to_string();
    let enrolled = repo
        .enroll_approved_exact(id, owner, "Joining Node", &endpoint, 123)
        .await
        .expect("exact approved enrollment");
    assert_eq!(enrolled.id, id);
    assert_eq!(enrolled.owner_subject, owner.to_string());
    assert_eq!(enrolled.created_at.timestamp(), 123);
    assert_eq!(
        repo.enroll_approved_exact(id, owner, "Joining Node", &endpoint, 456)
            .await
            .expect("exact retry preserves original timestamps"),
        enrolled
    );
    for (id, owner, name, endpoint) in [
        (Id::now_v7(), owner, "Joining Node", endpoint.clone()),
        (id, Id::now_v7(), "Joining Node", endpoint.clone()),
        (id, owner, "Changed Node", endpoint.clone()),
        (
            id,
            owner,
            "Joining Node",
            iroh::SecretKey::generate().public().to_string(),
        ),
    ] {
        assert!(
            repo.enroll_approved_exact(id, owner, name, &endpoint, 456)
                .await
                .is_err()
        );
    }
    assert_eq!(
        repo.find_approved_by_public_key(&endpoint)
            .await
            .expect("current Device"),
        Some(enrolled)
    );
    repo.revoke_self(&endpoint).await.expect("revoke Device");
    assert!(
        repo.enroll_approved_exact(id, owner, "Joining Node", &endpoint, 456)
            .await
            .is_err()
    );
    assert!(
        repo.find_approved_by_public_key(&endpoint)
            .await
            .expect("revoked binding")
            .is_none()
    );
}

#[cfg(feature = "replica")]
#[tokio::test]
async fn approved_enrollment_denies_tombstones_pending_and_timestamp_only_revocation() {
    use device_service::DeviceRepo;
    use device_service::replica::DbDeviceRepo;
    use idp_model::model::Id;
    use std::sync::Arc;

    let engine = Arc::new(db::Database::in_memory());
    device_service::replica::up(&engine)
        .await
        .expect("Management schema");
    let repo = DbDeviceRepo::new(engine.clone().client());
    let owner = Id::now_v7();
    let endpoint = iroh::SecretKey::generate().public().to_string();
    let id = Id::now_v7();
    repo.enroll_approved_exact(id, owner, "Joining Node", &endpoint, 123)
        .await
        .expect("approved binding");
    for mutation in [
        "UPDATE devices SET state = 0",
        "UPDATE devices SET state = 1, revoked_at = 1",
        "DELETE FROM devices",
    ] {
        engine
            .client()
            .translate_and_execute(
                &format!("{mutation} WHERE id = CAST('{id}' AS UUID)"),
                &db::SqlTranslator,
            )
            .await
            .expect("learn invalid enrollment state");
        assert!(
            repo.enroll_approved_exact(id, owner, "Joining Node", &endpoint, 456)
                .await
                .is_err()
        );
        assert!(
            repo.find_approved_by_public_key(&endpoint)
                .await
                .expect("current admission")
                .is_none()
        );
    }
}

#[cfg(feature = "replica")]
#[tokio::test]
async fn approved_enrollment_denies_unresolved_owner_conflicts_even_with_exact_visible_binding() {
    use device_service::replica::DbDeviceRepo;
    use idp_model::model::Id;
    use std::sync::Arc;

    let left = Arc::new(db::Database::in_memory());
    device_service::replica::up(&left)
        .await
        .expect("Management schema");
    let repo = DbDeviceRepo::new(left.clone().client());
    let id = Id::now_v7();
    let owner = Id::now_v7();
    let endpoint = iroh::SecretKey::generate().public().to_string();
    repo.enroll_approved_exact(id, owner, "Joining Node", &endpoint, 123)
        .await
        .expect("approved binding");
    let right = db::Database::in_memory();
    for unit in left
        .export_sync_state()
        .await
        .expect("initial public state")
    {
        right
            .apply_sync_state(unit)
            .await
            .expect("replicated Management state");
    }
    for (engine, address) in [(left.as_ref(), "left"), (&right, "right")] {
        engine
            .client()
            .translate_and_execute(
                &format!(
                    "UPDATE devices SET address = '{address}' WHERE id = CAST('{id}' AS UUID)"
                ),
                &db::SqlTranslator,
            )
            .await
            .expect("concurrent Management update");
    }
    for unit in right
        .export_sync_state()
        .await
        .expect("concurrent public state")
    {
        left.apply_sync_state(unit).await.expect("learn conflict");
    }
    assert!(
        !left
            .client()
            .row_conflicts("devices", &db::Row::new(vec![db::Value::Uuid(id)]))
            .await
            .expect("conflicted fixture")
            .is_empty()
    );
    assert!(
        repo.enroll_approved_exact(id, owner, "Joining Node", &endpoint, 456)
            .await
            .is_err()
    );
}
