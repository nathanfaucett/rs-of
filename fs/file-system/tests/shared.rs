use std::{env, fs};

use bytes::Bytes;
use file_system::{Error, FileSystemCatalog, Residency};
use ofdb_kv::SharedDatabase;
use uuid::Uuid;

#[tokio::test]
async fn shared_filesystem_retains_content_catalog_and_retirement_in_one_file() {
    let root = env::temp_dir().join(format!("offs-shared-{}", Uuid::now_v7()));
    fs::create_dir(&root).expect("host root");
    let path = root.join("unified.redb");
    let host = SharedDatabase::create(&path).expect("host");
    let catalog = FileSystemCatalog::open_shared(&host, "owner/app").expect("catalog");
    let resource = catalog
        .create(Some("documents".into()))
        .await
        .expect("resource");
    let fs = catalog
        .open_filesystem(resource.id, 1_u8)
        .await
        .expect("filesystem");
    fs.set_residency("", Residency::Full)
        .await
        .expect("full copy");
    let created = fs.create_file("created.txt").await.expect("create");
    assert_eq!(created.meta.mode, 0o644);
    assert!(matches!(
        fs.create_file("created.txt").await,
        Err(Error::AlreadyExists)
    ));
    assert_eq!(fs.read("created.txt").await.expect("empty create"), b"");
    fs.write("old.txt", b"saved").await.expect("write");
    fs.rename("old.txt", "saved.txt").await.expect("rename");
    fs.append("saved.txt", b" content").await.expect("append");
    drop(fs);
    drop(catalog);
    drop(host);
    let host = SharedDatabase::create(&path).expect("restart host");
    let catalog = FileSystemCatalog::open_shared(&host, "owner/app").expect("restart catalog");
    assert_eq!(
        catalog.list().await.expect("retained catalog"),
        vec![resource.clone()]
    );
    let fs = catalog
        .open_filesystem(resource.id, 1_u8)
        .await
        .expect("retained filesystem");
    assert_eq!(
        fs.residency("").expect("retained residency"),
        Residency::Full
    );
    assert_eq!(
        fs.local_content_len("saved.txt")
            .await
            .expect("retained content length"),
        13
    );
    assert_eq!(
        fs.read("saved.txt").await.expect("retained contents"),
        b"saved content"
    );
    assert_eq!(
        fs.read("created.txt").await.expect("retained empty create"),
        b""
    );
    assert_eq!(
        fs.entry("created.txt")
            .await
            .expect("created identity")
            .meta,
        created.meta
    );
    catalog
        .remove_managed_copy(resource.id)
        .await
        .expect("retire copy");
    fs.read("saved.txt").await.unwrap_err();
    catalog
        .open_filesystem(resource.id, 1_u8)
        .await
        .expect_err("retired filesystem remains closed");
    assert_eq!(
        catalog.list().await.expect("identity retained"),
        vec![resource.clone()]
    );
    assert!(path.is_file());
    assert_eq!(fs::read_dir(&root).expect("single file layout").count(), 1);
    drop(fs);
    drop(catalog);
    drop(host);
    let host = SharedDatabase::create(&path).expect("restart retired host");
    let catalog = FileSystemCatalog::open_shared(&host, "owner/app").expect("retained catalog");
    catalog
        .open_filesystem(resource.id, 1_u8)
        .await
        .expect_err("retired filesystem remains closed");
    catalog.delete(resource.id).await.expect("delete identity");
    assert!(catalog.snapshot().await.expect("tombstone")[0].deleted);
    drop(catalog);
    drop(host);
    fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn shared_filesystem_truncates_with_zero_fill_and_restart() {
    let root = env::temp_dir().join(format!("offs-shared-truncate-{}", Uuid::now_v7()));
    fs::create_dir(&root).expect("host root");
    let path = root.join("unified.redb");
    let host = SharedDatabase::create(&path).expect("host");
    let catalog = FileSystemCatalog::open_shared(&host, "owner/truncate").expect("catalog");
    let resource = catalog.create(None).await.expect("resource");
    let fs = catalog
        .open_filesystem(resource.id, 1_u8)
        .await
        .expect("filesystem");
    fs.set_residency("", Residency::Full).await.expect("full");
    let created = fs.write("file", b"hello world").await.expect("write");

    let shortened = fs
        .truncate("file", created.meta.revision, 5)
        .await
        .expect("shrink");
    assert_eq!(fs.read("file").await.expect("short content"), b"hello");
    let extended = fs.truncate("file", shortened, 9).await.expect("extend");
    assert_eq!(fs.read("file").await.expect("zero fill"), b"hello\0\0\0\0");
    let emptied = fs.truncate("file", extended, 0).await.expect("empty");
    assert_eq!(fs.read("file").await.expect("empty content"), b"");
    fs.write_at("file", emptied, 0, Bytes::from_static(b"saved"))
        .await
        .expect("subsequent write");
    let saved = fs.entry("file").await.expect("saved entry");
    assert_eq!(saved.meta.file_id, created.meta.file_id);
    drop(fs);
    drop(catalog);
    drop(host);

    let host = SharedDatabase::create(&path).expect("restart host");
    let catalog = FileSystemCatalog::open_shared(&host, "owner/truncate").expect("restart catalog");
    let fs = catalog
        .open_filesystem(resource.id, 1_u8)
        .await
        .expect("restart filesystem");
    assert_eq!(fs.read("file").await.expect("restart content"), b"saved");
    assert_eq!(
        fs.entry("file").await.expect("restart entry").meta,
        saved.meta
    );
    drop(fs);
    drop(catalog);
    drop(host);
    fs::remove_dir_all(root).expect("cleanup");
}
