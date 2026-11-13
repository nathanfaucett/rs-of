use std::sync::Arc;

use embedded_db::{MemoryDatabase, local_client};
use of_model::db::Database;

#[cfg(unix)]
#[tokio::test]
async fn embedded_database_is_private_and_rejects_unsafe_existing_files() {
    use embedded_db::EmbeddedDatabase;
    use std::os::unix::fs::{PermissionsExt, symlink};

    let root = tempfile::tempdir().expect("private database root");
    let path = root.path().join("unified.redb");
    let database = EmbeddedDatabase::open(root.path())
        .await
        .expect("open private database");
    assert_eq!(
        std::fs::metadata(&path)
            .expect("database metadata")
            .permissions()
            .mode()
            & 0o077,
        0
    );
    drop(database);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
        .expect("unsafe permissions");
    assert!(EmbeddedDatabase::open(root.path()).await.is_err());
    assert_eq!(
        std::fs::metadata(&path)
            .expect("retained metadata")
            .permissions()
            .mode()
            & 0o777,
        0o644
    );
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
        .expect("restore permissions");
    let linked = tempfile::tempdir().expect("symlink root");
    symlink(&path, linked.path().join("unified.redb")).expect("database symlink");
    assert!(EmbeddedDatabase::open(linked.path()).await.is_err());
}

#[tokio::test]
async fn local_clients_use_the_injected_database_without_owner_files() {
    let root = tempfile::tempdir().expect("local owner root");
    let database: Arc<dyn Database> = Arc::new(MemoryDatabase::new().await.expect("shared schema"));
    for owner in ["idp", "device", "storage", "management"] {
        let client =
            local_client(root.path(), owner, Some(database.clone())).expect("shared owner");
        client.execute_sql(&format!("INSERT INTO namespaces (id, owner_subject, application_id) VALUES (CAST('{}' AS UUID), '{}', CAST('{}' AS UUID))", of_model::idp::model::Id::now_v7(), owner, of_model::idp::model::Id::now_v7()), None).await.expect("write through owner client");
        assert!(!root.path().join(format!("{owner}.redb")).exists());
    }
    let result = database
        .client()
        .execute_sql("SELECT * FROM namespaces", None)
        .await
        .expect("shared writes");
    assert_eq!(result[0].rows.len(), 4);
}

#[test]
fn local_owner_rejects_unknown_names_and_preserves_invalid_files() {
    let root = tempfile::tempdir().expect("local owner root");
    assert!(local_client(root.path(), "../other", None).is_err());
    let path = root.path().join("idp.redb");
    std::fs::write(&path, b"not a database").expect("invalid input fixture");
    assert!(local_client(root.path(), "idp", None).is_err());
    assert_eq!(
        std::fs::read(path).expect("preserved invalid file"),
        b"not a database"
    );
}

#[tokio::test]
async fn local_owner_opens_only_its_named_file_and_initializes_schema() {
    let root = tempfile::tempdir().expect("local owner root");
    for owner in ["idp", "device", "storage", "management"] {
        let client = local_client(root.path(), owner, None).expect("standalone local owner");
        for table in ["users", "devices", "resources", "sync_selections"] {
            client
                .execute_sql(&format!("SELECT * FROM {table}"), None)
                .await
                .expect("owner schema");
        }
        assert!(root.path().join(format!("{owner}.redb")).is_file());
        drop(client);
        local_client(root.path(), owner, None).expect("reopen owner");
    }
    assert!(!root.path().join("unified.redb").exists());
}
