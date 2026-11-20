#![cfg(feature = "ofdb")]

use idp_service::{ofdb::DbUserRepo, password_config::PasswordConfig, repo::UserRepo};
use of_model::id::Id;
use ofdb_sql::{Database, SyncKey};

#[tokio::test]
async fn stable_user_and_credential_ids_are_preserved_without_choosing_logical_rows() {
    let source = Database::in_memory();
    db::ofdb::up(&source.client())
        .await
        .expect("fresh IdP schema");
    let subject = Id::new_v5(&Id::NAMESPACE_URL, b"of:test:stable-user");
    let credential = Id::new_v5(&subject, b"password");
    let users = DbUserRepo::new(source.client(), PasswordConfig::default());
    let user = users
        .create_user_with_password_and_ids(subject, credential, "seeded", "test password")
        .await
        .expect("create seeded User and Credential");
    assert_eq!(user.id, subject);

    let mut history = source.export_history().await.expect("export IdP history");
    for (name, public_id) in [("users", subject), ("credentials", credential)] {
        let row = history
            .iter()
            .find_map(|unit| match &unit.key {
                SyncKey::Row { table, row } if table == name => Some(*row),
                _ => None,
            })
            .expect("record in IdP history");
        assert_eq!(row.get_version_num(), 7);
        assert_ne!(row, public_id);
    }

    // Private password verifiers stay on their original Node.
    history.retain(|unit| !matches!(&unit.key, SyncKey::Row { table, .. } if table == "local_credential_secrets"));
    let replica = Database::in_memory();
    replica
        .import_history(history)
        .await
        .expect("apply Replica snapshot");
    let replicated_users = DbUserRepo::new(replica.client(), PasswordConfig::default());
    assert_eq!(
        replicated_users
            .find_user_by_id(subject)
            .await
            .expect("lookup on Replica")
            .expect("public User")
            .id,
        subject
    );
    assert!(
        replicated_users
            .find_user_password_by_user_id(subject)
            .await
            .expect("check Replica credential")
            .is_none()
    );
    assert_eq!(
        users
            .find_user_password_by_user_id(subject)
            .await
            .expect("lookup local password")
            .expect("local Credential")
            .id,
        credential
    );
}
