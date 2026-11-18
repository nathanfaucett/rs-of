use std::{env, fs};

use file_system::{CatalogEntry, Error, FileSystemCatalog, FileSystemId};
use uuid::Uuid;

fn root() -> std::path::PathBuf {
    env::temp_dir().join(format!("file-system-catalog-{}", Uuid::now_v7()))
}

#[tokio::test]
async fn resources_have_distinct_roots_duplicate_names_and_persist() {
    let root = root();
    let catalog = FileSystemCatalog::open(&root).expect("open catalog");
    let first = catalog
        .create(Some("same".to_owned()))
        .await
        .expect("create first");
    let second = catalog
        .create(Some("same".to_owned()))
        .await
        .expect("create second");
    let first_id = first.id;
    assert_ne!(first.id, second.id);
    catalog
        .open_filesystem(first.id, 1_u8)
        .await
        .expect("open first FS");
    catalog
        .open_filesystem(second.id, 1_u8)
        .await
        .expect("open second FS");
    assert!(
        root.join("filesystems")
            .join(first.id.as_uuid().to_string())
            .exists()
    );
    assert!(
        root.join("filesystems")
            .join(second.id.as_uuid().to_string())
            .exists()
    );
    drop(catalog);

    let reopened = FileSystemCatalog::open(&root).expect("reopen catalog");
    assert_eq!(
        reopened.list().await.expect("list resources"),
        vec![first, second]
    );
    reopened.open_filesystem(first_id, 1_u8).await.unwrap();
    fs::remove_dir_all(root).expect("remove test root");
}

#[tokio::test]
async fn deletion_is_a_durable_tombstone_exported_in_snapshot() {
    let root = root();
    let catalog = FileSystemCatalog::open(&root).expect("open catalog");
    let resource = catalog.create(None).await.expect("create resource");
    let filesystem_root = root
        .join("filesystems")
        .join(resource.id.as_uuid().to_string());
    catalog
        .open_filesystem(resource.id, 1_u8)
        .await
        .expect("open filesystem");
    catalog.delete(resource.id).await.expect("delete resource");
    assert!(filesystem_root.exists());
    assert!(matches!(
        catalog.open_filesystem(resource.id, 1_u8).await,
        Err(Error::NotFound)
    ));
    assert_eq!(catalog.list().await.expect("list resources"), [] as [file_system::FileSystemResource; 0]);
    let snapshot = catalog.snapshot().await.expect("export snapshot");
    assert_eq!(snapshot.len(), 1);
    assert!(snapshot[0].deleted);
    drop(catalog);

    let reopened = FileSystemCatalog::open(&root).expect("reopen catalog");
    let imported_root = root.join("other-namespace");
    let imported = FileSystemCatalog::open(&imported_root).expect("open import catalog");
    imported
        .import_snapshot(&reopened.snapshot().await.expect("snapshot after restart"))
        .await
        .expect("import snapshot");
    assert_eq!(imported.list().await.expect("list imported"), [] as [file_system::FileSystemResource; 0]);
    assert!(matches!(
        imported.open_filesystem(resource.id, 1_u8).await,
        Err(Error::NotFound)
    ));
    assert!(matches!(
        imported.delete(resource.id).await,
        Err(Error::NotFound)
    ));
    fs::remove_dir_all(root).expect("remove test root");
}

#[tokio::test]
async fn imported_tombstone_wins_over_a_live_catalog_entry() {
    let root = root();
    let catalog = FileSystemCatalog::open(&root).expect("open catalog");
    let resource = catalog
        .create(Some("duplicate ok".to_owned()))
        .await
        .expect("create resource");
    let tombstone = CatalogEntry {
        resource: resource.clone(),
        deleted: true,
    };
    catalog
        .import_snapshot(&[tombstone])
        .await
        .expect("import tombstone");
    catalog
        .import_snapshot(&[CatalogEntry {
            resource: resource.clone(),
            deleted: false,
        }])
        .await
        .expect("import stale live record");
    assert_eq!(catalog.list().await.expect("list resources"), [] as [file_system::FileSystemResource; 0]);
    fs::remove_dir_all(root).expect("remove test root");
}

#[tokio::test]
async fn deselecting_projected_resource_evicts_only_local_copy_without_tombstone() {
    let root = root();
    let catalog = FileSystemCatalog::open(&root).expect("open catalog");
    let resource = catalog
        .create(Some("shared".to_owned()))
        .await
        .expect("create resource");
    catalog
        .mark_projected(resource.id)
        .await
        .expect("mark projection");
    let filesystem = catalog
        .open_filesystem(resource.id, 1_u8)
        .await
        .expect("open projected filesystem");
    filesystem
        .set_residency("", file_system::Residency::Full)
        .await
        .expect("set full residency");
    filesystem
        .write("cached.txt", b"local copy")
        .await
        .expect("write local copy");
    let local_root = root
        .join("filesystems")
        .join(resource.id.as_uuid().to_string());
    assert!(local_root.exists());

    assert!(
        catalog
            .evict_projected_copy(resource.id)
            .await
            .expect("evict copy")
    );
    assert!(!local_root.exists());
    assert_eq!(
        catalog.list().await.expect("catalog retains identity"),
        [resource]
    );
    assert!(!catalog.snapshot().await.expect("snapshot remains live")[0].deleted);
    assert_eq!(
        catalog
            .projected_selected()
            .await
            .expect("projection deselected"), [] as [file_system::FileSystemId; 0]
    );

    drop(filesystem);
    drop(catalog);
    fs::remove_dir_all(root).expect("remove test root");
}

#[tokio::test]
async fn filesystem_id_is_uuid_v7_backed_and_parsed_ids_are_checked() {
    let id = FileSystemId::new();
    assert_eq!(id.as_uuid().get_version_num(), 7);
    assert_eq!(
        FileSystemId::parse(&id.as_uuid().to_string()).expect("parse UUIDv7"),
        id
    );
    FileSystemId::parse("not-an-id").unwrap_err();
    FileSystemId::parse(&Uuid::nil().to_string()).unwrap_err();
}
