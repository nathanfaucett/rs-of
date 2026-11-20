use std::{env, fs, io, path::PathBuf};

use file_system::{Error, FileSystem, Residency};
use file_system_fuse::{MountError, mount};
use uuid::Uuid;

fn root(name: &str) -> PathBuf {
    env::temp_dir().join(format!("offs-fuse-{name}-{}", Uuid::now_v7()))
}

#[tokio::test]
async fn mount_rejects_default_passthrough_without_changing_residency() {
    let backing = root("backing");
    let mountpoint = root("mount");
    fs::create_dir(&mountpoint).expect("mountpoint");
    let file_system = FileSystem::open(&backing, 1_u8).await.expect("open");
    let retained = file_system.clone();

    let result = mount(file_system, &mountpoint).await;

    assert!(matches!(
        result,
        Err(MountError::NotFullResidency { path }) if path.is_empty()
    ));
    assert_eq!(
        retained.residency("").expect("residency"),
        Residency::Passthrough
    );
    assert_eq!(fs::read_dir(&mountpoint).expect("mountpoint").count(), 0);
    fs::remove_dir_all(backing).expect("backing cleanup");
    fs::remove_dir_all(mountpoint).expect("mount cleanup");
}

#[tokio::test]
async fn mount_rejects_nested_passthrough_without_evicting_other_content() {
    let backing = root("backing");
    let mountpoint = root("mount");
    fs::create_dir(&mountpoint).expect("mountpoint");
    let file_system = FileSystem::open(&backing, 1_u8).await.expect("open");
    file_system
        .set_residency("", Residency::Full)
        .await
        .expect("full root");
    file_system
        .write("root.txt", b"retained")
        .await
        .expect("write");
    file_system.create_dir("nested").await.expect("directory");
    file_system
        .set_residency("nested", Residency::Passthrough)
        .await
        .expect("nested passthrough");
    let retained = file_system.clone();

    let result = mount(file_system, &mountpoint).await;

    assert!(matches!(
        result,
        Err(MountError::NotFullResidency { path }) if path == "nested"
    ));
    assert_eq!(
        retained.residency("nested").expect("residency"),
        Residency::Passthrough
    );
    assert_eq!(
        retained.read("root.txt").await.expect("content"),
        b"retained"
    );
    fs::remove_dir_all(backing).expect("backing cleanup");
    fs::remove_dir_all(mountpoint).expect("mount cleanup");
}

#[tokio::test]
async fn mount_rejects_incomplete_full_content_without_changing_revision() {
    let backing = root("backing");
    let mountpoint = root("mount");
    fs::create_dir(&mountpoint).expect("mountpoint");
    let file_system = FileSystem::open(&backing, 1_u8).await.expect("open");
    file_system
        .set_residency("", Residency::Full)
        .await
        .expect("full root");
    let entry = file_system
        .write("file.txt", b"missing")
        .await
        .expect("write");
    fs::remove_file(
        backing
            .join(".data")
            .join(entry.meta.file_id.to_string())
            .join(entry.meta.revision.to_string()),
    )
    .expect("remove retained revision");
    let retained = file_system.clone();

    let result = mount(file_system, &mountpoint).await;

    assert!(matches!(
        result,
        Err(MountError::FileSystem(Error::ContentUnavailable))
    ));
    assert_eq!(
        retained.residency("file.txt").expect("residency"),
        Residency::Full
    );
    assert_eq!(
        retained.revision("file.txt").await.expect("revision"),
        entry.meta.revision
    );
    assert!(matches!(
        retained.local_content_len("file.txt").await,
        Err(Error::ContentUnavailable)
    ));
    fs::remove_dir_all(backing).expect("backing cleanup");
    fs::remove_dir_all(mountpoint).expect("mount cleanup");
}

#[tokio::test]
async fn mount_rejects_the_nonempty_backing_root_as_mountpoint() {
    let backing = root("backing");
    let file_system = FileSystem::open(&backing, 1_u8).await.expect("open");
    file_system
        .set_residency("", Residency::Full)
        .await
        .expect("full root");
    file_system
        .write("file.txt", b"retained")
        .await
        .expect("write");
    let retained = file_system.clone();

    let result = mount(file_system, &backing).await;

    assert!(matches!(
        result,
        Err(MountError::Io(error)) if error.kind() == io::ErrorKind::InvalidInput
    ));
    assert_eq!(
        retained.read("file.txt").await.expect("content"),
        b"retained"
    );
    fs::remove_dir_all(backing).expect("backing cleanup");
}
