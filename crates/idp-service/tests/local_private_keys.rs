#![cfg(feature = "replica")]

use idp_service::repo::{PrivateKeyKeyringRepo, PrivateKeyRepo};
use key::DerivationPath;

#[test]
fn local_private_keys_persist_without_an_os_credential_store() {
    let root = std::env::temp_dir().join(format!(
        "local-private-{}",
        of_model::idp::model::Id::now_v7()
    ));
    std::fs::create_dir(&root).expect("database directory");
    let database = db::Database::open(root.join("unified.redb")).expect("database");
    let repo =
        PrivateKeyKeyringRepo::new_local(database.client()).expect("database-backed local keys");
    let key = repo.ensure_master_key("local").expect("create local key");
    drop(repo);
    drop(database);
    let database = db::Database::open(root.join("unified.redb")).expect("reopen database");
    let reopened = PrivateKeyKeyringRepo::new_local(database.client()).expect("reopen local keys");
    assert_eq!(
        reopened
            .load("local", &DerivationPath::default())
            .expect("read key")
            .expect("retained key")
            .to_xprv_string(),
        key.to_xprv_string()
    );
    assert!(
        reopened
            .load("other", &DerivationPath::default())
            .expect("separate namespace")
            .is_none()
    );
    reopened
        .delete("local", &DerivationPath::default())
        .expect("delete local key");
    assert!(
        reopened
            .load("local", &DerivationPath::default())
            .expect("deleted key")
            .is_none()
    );
    drop(reopened);
    drop(database);
    let database = db::Database::open(root.join("unified.redb")).expect("reopen after delete");
    let reopened =
        PrivateKeyKeyringRepo::new_local(database.client()).expect("retained key deletion");
    assert!(
        reopened
            .load("local", &DerivationPath::default())
            .expect("deleted after restart")
            .is_none()
    );
    let replacement = reopened
        .ensure_master_key("local")
        .expect("replace deleted key");
    assert_ne!(replacement.to_xprv_string(), key.to_xprv_string());
    assert_eq!(
        std::fs::read_dir(&root).expect("local disk layout").count(),
        1
    );
    drop(reopened);
    drop(database);
    std::fs::remove_dir_all(root).expect("remove local keys");
}
