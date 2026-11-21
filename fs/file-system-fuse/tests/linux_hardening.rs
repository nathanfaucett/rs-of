use std::{
    env, fs,
    os::unix::fs::FileExt,
    process::{Command, ExitStatus},
    thread,
    time::Duration,
};

use file_system::{
    Error, FileSystem, FileSystemCatalog, MemoryNetwork, OperationControl, Residency,
};
use file_system_fuse::mount;
use tokio::{
    task::{spawn, yield_now},
    time::timeout,
};
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires /dev/fuse and unprivileged Linux mount support"]
async fn unmount_closes_open_file_and_directory_handles() {
    let unique = Uuid::now_v7();
    let backing = env::temp_dir().join(format!("offs-fuse-shutdown-backing-{unique}"));
    let mountpoint = env::temp_dir().join(format!("offs-fuse-shutdown-mount-{unique}"));
    fs::create_dir(&mountpoint).expect("mountpoint");
    let file_system = FileSystem::open(&backing, 1_u8).await.expect("open");
    file_system
        .set_residency("", Residency::Full)
        .await
        .expect("full root");
    file_system
        .write("file.txt", b"content")
        .await
        .expect("file");
    file_system.create_dir("dir").await.expect("directory");
    let retained = file_system.clone();
    let guard = mount(file_system, &mountpoint).await.expect("mount");
    let file = fs::File::open(mountpoint.join("file.txt")).expect("open file");
    let directory = fs::read_dir(mountpoint.join("dir")).expect("open directory");

    let unmount = timeout(Duration::from_secs(5), guard.unmount()).await;
    let mut byte = [0_u8; 1];
    let read_after_unmount = matches!(unmount, Ok(Ok(()))).then(|| file.read_at(&mut byte, 0));
    drop(file);
    drop(directory);

    let force_unmount = if matches!(unmount, Ok(Ok(()))) {
        None
    } else {
        Some(
            Command::new("fusermount3")
                .arg("-u")
                .arg(&mountpoint)
                .status(),
        )
    };
    drop(retained);
    if force_unmount
        .as_ref()
        .is_none_or(|result| result.as_ref().is_ok_and(ExitStatus::success))
    {
        fs::remove_dir_all(&mountpoint).expect("mount cleanup");
    }
    fs::remove_dir_all(backing).expect("backing cleanup");

    assert!(matches!(unmount, Ok(Ok(()))), "unmount did not finish");
    assert!(
        read_after_unmount.is_some_and(|result| result.is_err()),
        "an open file remained usable after unmount"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires /dev/fuse and unprivileged Linux mount support"]
async fn learned_retirement_denies_open_handles_and_unmounts() {
    let unique = Uuid::now_v7();
    let backing = env::temp_dir().join(format!("offs-fuse-retired-backing-{unique}"));
    let mountpoint = env::temp_dir().join(format!("offs-fuse-retired-mount-{unique}"));
    fs::create_dir(&mountpoint).expect("mountpoint");
    let catalog = FileSystemCatalog::open(&backing).expect("catalog");
    let resource = catalog.create(None).await.expect("resource");
    catalog
        .mark_projected(resource.id)
        .await
        .expect("projected resource");
    let file_system = catalog
        .open_filesystem(resource.id, 1_u8)
        .await
        .expect("filesystem");
    file_system
        .set_residency("", Residency::Full)
        .await
        .expect("full root");
    file_system
        .write("file.txt", b"content")
        .await
        .expect("file");
    let guard = mount(file_system, &mountpoint).await.expect("mount");
    let file = fs::File::open(mountpoint.join("file.txt")).expect("open file");

    let retired = catalog.evict_projected_copy(resource.id).await;
    let mut byte = [0_u8; 1];
    let denied = file.read_at(&mut byte, 0);
    let unmount = timeout(Duration::from_secs(5), guard.unmount()).await;
    drop(file);
    drop(catalog);
    if matches!(unmount, Ok(Ok(()))) {
        fs::remove_dir_all(&mountpoint).expect("mount cleanup");
    } else {
        let _ = Command::new("fusermount3")
            .arg("-uz")
            .arg(&mountpoint)
            .status();
        let _ = fs::remove_dir_all(&mountpoint);
    }
    if backing.exists() {
        fs::remove_dir_all(backing).expect("backing cleanup");
    }

    assert!(retired.expect("retirement"));
    assert_eq!(
        denied.map_err(|error| error.raw_os_error()),
        Err(Some(libc::EACCES))
    );
    assert!(matches!(unmount, Ok(Ok(()))), "unmount did not finish");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires /dev/fuse and unprivileged Linux mount support"]
async fn unmount_does_not_retain_the_callers_pending_synchronization() {
    let unique = Uuid::now_v7();
    let backing = env::temp_dir().join(format!("offs-fuse-sync-backing-{unique}"));
    let mountpoint = env::temp_dir().join(format!("offs-fuse-sync-mount-{unique}"));
    fs::create_dir(&mountpoint).expect("mountpoint");
    let file_system = FileSystem::open(&backing, 1_u8).await.expect("open");
    file_system
        .set_residency("", Residency::Full)
        .await
        .expect("full root");
    file_system.write("file", b"local").await.expect("file");
    let sync_file_system = file_system.clone();
    let guard = mount(file_system, &mountpoint).await.expect("mount");
    let network = MemoryNetwork::new(16);
    let transport = network.transport(1_u8);
    let control = OperationControl::new(|_, _| Box::pin(async { true }));
    let task_control = control.clone();
    let synchronization = spawn(async move {
        sync_file_system
            .metadata_sync(transport)
            .with_operation_control(task_control)
            .run()
            .await
    });
    yield_now().await;

    let unmount = timeout(Duration::from_secs(5), guard.unmount()).await;
    control.cancel();
    let synchronization = timeout(Duration::from_secs(5), synchronization).await;
    drop(network);
    if matches!(unmount, Ok(Ok(()))) {
        fs::remove_dir_all(&mountpoint).expect("mount cleanup");
    } else {
        let _ = Command::new("fusermount3")
            .arg("-uz")
            .arg(&mountpoint)
            .status();
        let _ = fs::remove_dir_all(&mountpoint);
    }
    fs::remove_dir_all(backing).expect("backing cleanup");

    assert!(matches!(unmount, Ok(Ok(()))), "unmount did not finish");
    assert!(matches!(synchronization, Ok(Ok(Err(Error::Cancelled)))));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires /dev/fuse and unprivileged Linux mount support"]
async fn concurrent_processes_preserve_offset_and_append_writes() {
    let unique = Uuid::now_v7();
    let backing = env::temp_dir().join(format!("offs-fuse-process-backing-{unique}"));
    let mountpoint = env::temp_dir().join(format!("offs-fuse-process-mount-{unique}"));
    fs::create_dir(&mountpoint).expect("mountpoint");
    let file_system = FileSystem::open(&backing, 1_u8).await.expect("open");
    file_system
        .set_residency("", Residency::Full)
        .await
        .expect("full root");
    let original = file_system
        .write("file.txt", b"abcdefgh")
        .await
        .expect("initial content");
    let retained = file_system.clone();
    let guard = mount(file_system, &mountpoint).await.expect("mount");
    let path = mountpoint.join("file.txt");

    let mut left = Command::new("sh")
        .arg("-c")
        .arg("printf X | dd of=\"$1\" bs=1 seek=0 conv=notrunc status=none")
        .arg("sh")
        .arg(&path)
        .spawn()
        .expect("left offset process");
    let mut right = Command::new("sh")
        .arg("-c")
        .arg("printf Y | dd of=\"$1\" bs=1 seek=1 conv=notrunc status=none")
        .arg("sh")
        .arg(&path)
        .spawn()
        .expect("right offset process");
    let left_offset = left.wait();
    let right_offset = right.wait();

    let mut left = Command::new("sh")
        .arg("-c")
        .arg("printf A >> \"$1\"")
        .arg("sh")
        .arg(&path)
        .spawn()
        .expect("left append process");
    let mut right = Command::new("sh")
        .arg("-c")
        .arg("printf B >> \"$1\"")
        .arg("sh")
        .arg(&path)
        .spawn()
        .expect("right append process");
    let left_append = left.wait();
    let right_append = right.wait();

    let final_entry = retained.entry("file.txt").await.expect("final entry");
    let content = retained.read("file.txt").await.expect("final content");
    guard.unmount().await.expect("unmount");
    drop(retained);
    fs::remove_dir_all(backing).expect("backing cleanup");
    fs::remove_dir_all(mountpoint).expect("mount cleanup");

    for status in [left_offset, right_offset, left_append, right_append] {
        assert!(status.expect("process status").success());
    }
    assert_eq!(final_entry.meta.file_id, original.meta.file_id);
    assert_ne!(final_entry.meta.revision, original.meta.revision);
    assert_eq!(content.len(), 10);
    assert_eq!(&content[..8], b"XYcdefgh");
    let suffix = &content[8..];
    assert!(suffix == b"AB" || suffix == b"BA");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires /dev/fuse and unprivileged Linux mount support"]
async fn concurrent_namespace_mutations_preserve_final_state() {
    let unique = Uuid::now_v7();
    let backing = env::temp_dir().join(format!("offs-fuse-namespace-backing-{unique}"));
    let mountpoint = env::temp_dir().join(format!("offs-fuse-namespace-mount-{unique}"));
    fs::create_dir(&mountpoint).expect("mountpoint");
    let file_system = FileSystem::open(&backing, 1_u8).await.expect("open");
    file_system
        .set_residency("", Residency::Full)
        .await
        .expect("full root");
    file_system
        .write("source", b"content")
        .await
        .expect("source file");
    file_system
        .create_dir("source-dir")
        .await
        .expect("source directory");
    let retained = file_system.clone();
    let guard = mount(file_system, &mountpoint).await.expect("mount");

    let (rename_file, unlink_file, rename_dir, rmdir_dir) = thread::scope(|scope| {
        let rename_file =
            scope.spawn(|| fs::rename(mountpoint.join("source"), mountpoint.join("moved")));
        let unlink_file = scope.spawn(|| fs::remove_file(mountpoint.join("source")));
        let rename_dir =
            scope.spawn(|| fs::rename(mountpoint.join("source-dir"), mountpoint.join("moved-dir")));
        let rmdir_dir = scope.spawn(|| fs::remove_dir(mountpoint.join("source-dir")));
        (
            rename_file.join().expect("file rename"),
            unlink_file.join().expect("file unlink"),
            rename_dir.join().expect("directory rename"),
            rmdir_dir.join().expect("directory removal"),
        )
    });

    let source_exists = retained.entry("source").await.is_ok();
    let moved_content = retained.read("moved").await.ok();
    let source_dir_exists = retained.entry("source-dir").await.is_ok();
    let moved_dir_exists = retained.entry("moved-dir").await.is_ok();
    guard.unmount().await.expect("unmount");
    drop(retained);
    fs::remove_dir_all(backing).expect("backing cleanup");
    fs::remove_dir_all(mountpoint).expect("mount cleanup");

    assert_ne!(rename_file.is_ok(), unlink_file.is_ok());
    assert!(!source_exists);
    if rename_file.is_ok() {
        assert_eq!(moved_content.as_deref(), Some(b"content" as &[u8]));
    } else {
        assert_eq!(moved_content, None);
    }
    assert_ne!(rename_dir.is_ok(), rmdir_dir.is_ok());
    assert!(!source_dir_exists);
    assert_eq!(moved_dir_exists, rename_dir.is_ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires /dev/fuse and unprivileged Linux mount support"]
async fn truncate_and_external_revisions_update_or_reject_every_handle() {
    let unique = Uuid::now_v7();
    let backing = env::temp_dir().join(format!("offs-fuse-revisions-backing-{unique}"));
    let mountpoint = env::temp_dir().join(format!("offs-fuse-revisions-mount-{unique}"));
    fs::create_dir(&mountpoint).expect("mountpoint");
    let file_system = FileSystem::open(&backing, 1_u8).await.expect("open");
    file_system
        .set_residency("", Residency::Full)
        .await
        .expect("full root");
    file_system
        .write("file.txt", b"abcdef")
        .await
        .expect("initial content");
    let retained = file_system.clone();
    let guard = mount(file_system, &mountpoint).await.expect("mount");
    let path = mountpoint.join("file.txt");
    let first = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .expect("first handle");
    let second = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .expect("second handle");

    let truncated = first.set_len(4);
    let write_after_truncate = second.write_at(b"Z", 3);
    let truncating_open = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&path)
        .expect("truncating open");
    drop(truncating_open);
    let write_after_open = second.write_at(b"Q", 0);
    let external = retained.write("file.txt", b"external").await;
    let external_write = second.write_at(b"!", 0);
    let mut byte = [0_u8; 1];
    let external_read = first.read_at(&mut byte, 0);
    drop(first);
    drop(second);

    let content = retained.read("file.txt").await.expect("final content");
    guard.unmount().await.expect("unmount");
    drop(retained);
    fs::remove_dir_all(backing).expect("backing cleanup");
    fs::remove_dir_all(mountpoint).expect("mount cleanup");

    assert_eq!(truncated.map_err(|error| error.raw_os_error()), Ok(()));
    assert_eq!(
        write_after_truncate.map_err(|error| error.raw_os_error()),
        Ok(1)
    );
    assert_eq!(
        write_after_open.map_err(|error| error.raw_os_error()),
        Ok(1)
    );
    external.expect("external revision");
    assert_eq!(
        external_write.map_err(|error| error.raw_os_error()),
        Err(Some(libc::ESTALE))
    );
    assert_eq!(
        external_read.map_err(|error| error.raw_os_error()),
        Err(Some(libc::ESTALE))
    );
    assert_eq!(content, b"external");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires /dev/fuse and unprivileged Linux mount support"]
async fn multiple_open_handles_share_admitted_revisions() {
    let unique = Uuid::now_v7();
    let backing = env::temp_dir().join(format!("offs-fuse-handles-backing-{unique}"));
    let mountpoint = env::temp_dir().join(format!("offs-fuse-handles-mount-{unique}"));
    fs::create_dir(&mountpoint).expect("mountpoint");
    let file_system = FileSystem::open(&backing, 1_u8).await.expect("open");
    file_system
        .set_residency("", Residency::Full)
        .await
        .expect("full root");
    file_system
        .write("file.txt", b"abcdefgh")
        .await
        .expect("initial content");
    let retained = file_system.clone();
    let guard = mount(file_system, &mountpoint).await.expect("mount");

    let first = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(mountpoint.join("file.txt"))
        .expect("first handle");
    let second = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(mountpoint.join("file.txt"))
        .expect("second handle");
    let first_write = first.write_at(b"A", 0);
    let second_write = second.write_at(b"B", 1);
    drop(first);
    drop(second);

    guard.unmount().await.expect("unmount");
    let content = retained.read("file.txt").await.expect("final content");
    drop(retained);
    fs::remove_dir_all(backing).expect("backing cleanup");
    fs::remove_dir_all(mountpoint).expect("mount cleanup");

    assert_eq!(first_write.map_err(|error| error.raw_os_error()), Ok(1));
    assert_eq!(second_write.map_err(|error| error.raw_os_error()), Ok(1));
    assert_eq!(content, b"ABcdefgh");
}
