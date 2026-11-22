#![cfg(feature = "ofdb")]

use idp_service::{ofdb::DbUserRepo, password_config::PasswordConfig, repo::UserRepo};
use of_model::id::Id;
use ofdb_sql::{Database, SyncKey};

#[tokio::test]
async fn stable_user_subject_is_preserved_without_choosing_the_logical_row() {
    let source = Database::in_memory();
    db::ofdb::up(&source.client())
        .await
        .expect("fresh IdP schema");
    let subject = Id::new_v5(&Id::NAMESPACE_URL, b"of:test:stable-user");
    let users = DbUserRepo::new(source.client(), PasswordConfig::default());
    let user = users
        .create_user_with_password_and_id(subject, "seeded", "test password")
        .await
        .expect("create seeded User");
    assert_eq!(user.id, subject);

    let mut history = source.export_history().await.expect("export IdP history");
    let row = history
        .iter()
        .find_map(|unit| match &unit.key {
            SyncKey::Row { table, row } if table == "users" => Some(*row),
            _ => None,
        })
        .expect("User record in IdP history");
    assert_eq!(row.get_version_num(), 7);
    assert_ne!(row, subject);

    // Import only the User history; native peer exchange has its own test.
    history.retain(|unit| matches!(&unit.key, SyncKey::Row { table, .. } if table == "users"));
    let replica = Database::in_memory();
    db::ofdb::up(&replica.client())
        .await
        .expect("fresh replica schema");
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
            .find_user_password_hash(subject)
            .await
            .expect("check Replica credential")
            .is_none()
    );
    assert!(
        users
            .find_user_password_hash(subject)
            .await
            .expect("read local password")
            .is_some()
    );
}
