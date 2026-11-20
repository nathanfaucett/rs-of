use std::{
    env,
    ffi::{CStr, CString, OsString},
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::{MetadataExt, PermissionsExt},
    },
    path::Path,
};

use file_system::{FileSystem, Residency};
use file_system_fuse::mount;
use nix::unistd::{getgid, getuid};
use uuid::Uuid;

struct Directory(*mut libc::DIR);

impl Directory {
    fn open(path: &Path) -> io::Result<Self> {
        let path = CString::new(path.as_os_str().as_bytes())?;
        // SAFETY: `path` is a terminated C string; the returned pointer is checked.
        let raw = unsafe { libc::opendir(path.as_ptr()) };
        if raw.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(raw))
    }

    fn entries(&self, offset: i64) -> io::Result<Vec<(String, i64)>> {
        // SAFETY: `self` owns a live directory and `offset` is zero or a prior entry offset.
        unsafe { libc::seekdir(self.0, offset) };
        let mut entries = Vec::new();
        loop {
            // SAFETY: Linux exposes thread-local errno for this thread.
            unsafe { *libc::__errno_location() = 0 };
            // SAFETY: `self` remains live until this loop finishes.
            let entry = unsafe { libc::readdir(self.0) };
            if entry.is_null() {
                // SAFETY: Linux exposes thread-local errno for this thread.
                let error = unsafe { *libc::__errno_location() };
                if error == 0 {
                    break;
                }
                return Err(io::Error::from_raw_os_error(error));
            }
            // SAFETY: `readdir` returned a valid entry until its next call.
            let entry = unsafe { &*entry };
            // SAFETY: `d_name` is NUL-terminated within the returned entry.
            let name = unsafe { CStr::from_ptr(entry.d_name.as_ptr()) }
                .to_str()
                .expect("UTF-8 fixture name")
                .to_owned();
            entries.push((name, entry.d_off));
        }
        Ok(entries)
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        // SAFETY: `opendir` returned this live pointer, and this owner closes it once.
        unsafe { libc::closedir(self.0) };
    }
}

fn directory_entries(path: &Path, offset: i64) -> io::Result<Vec<(String, i64)>> {
    Directory::open(path)?.entries(offset)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires /dev/fuse and unprivileged Linux mount support"]
#[allow(clippy::too_many_lines, reason = "one real-mount acceptance scenario")]
async fn linux_mount_supports_read_attributes_readdir_and_clean_unmount() {
    let unique = Uuid::now_v7();
    let backing = env::temp_dir().join(format!("offs-fuse-backing-{unique}"));
    let mountpoint = env::temp_dir().join(format!("offs-fuse-mount-{unique}"));
    fs::create_dir(&mountpoint).expect("mountpoint");
    let file_system = FileSystem::open(&backing, 1_u8).await.expect("open");
    file_system
        .set_residency("", Residency::Full)
        .await
        .expect("full root");
    file_system
        .write("root.txt", b"hello world")
        .await
        .expect("root file");
    file_system.create_dir("dir").await.expect("directory");
    file_system
        .write("dir/a.txt", b"nested")
        .await
        .expect("nested file");
    file_system
        .write("unavailable.txt", b"local")
        .await
        .expect("unavailable fixture");

    let retained = file_system.clone();
    let guard = mount(file_system, &mountpoint).await.expect("mount");
    assert_eq!(guard.mountpoint(), mountpoint);

    let root_metadata = fs::metadata(&mountpoint).expect("root attributes");
    let file_metadata = fs::metadata(mountpoint.join("root.txt")).expect("file attributes");
    assert!(root_metadata.is_dir());
    assert!(file_metadata.is_file());
    assert_eq!(file_metadata.len(), 11);
    assert_eq!(root_metadata.permissions().mode() & 0o777, 0o755);
    assert_eq!(file_metadata.permissions().mode() & 0o777, 0o644);
    assert_eq!(root_metadata.uid(), getuid().as_raw());
    assert_eq!(root_metadata.gid(), getgid().as_raw());
    assert_eq!(
        root_metadata.modified().expect("root mtime"),
        file_metadata.modified().expect("file mtime")
    );

    let mut file = File::open(mountpoint.join("root.txt")).expect("open file");
    file.seek(SeekFrom::Start(6)).expect("seek");
    let mut content = String::new();
    file.read_to_string(&mut content).expect("offset read");
    assert_eq!(content, "world");
    drop(file);

    let entries = directory_entries(&mountpoint, 0).expect("directory entries");
    let names = entries
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, [".", "..", "dir", "root.txt", "unavailable.txt"]);
    let resumed = directory_entries(&mountpoint, entries[1].1).expect("resume directory");
    assert_eq!(
        resumed
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        ["dir", "root.txt", "unavailable.txt"]
    );

    assert_eq!(
        fs::read(mountpoint.join("missing"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ENOENT)
    );
    assert_eq!(
        fs::read_dir(mountpoint.join("root.txt"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ENOTDIR)
    );
    assert_eq!(
        fs::read(&mountpoint).unwrap_err().raw_os_error(),
        Some(libc::EISDIR)
    );
    assert_eq!(
        fs::metadata(mountpoint.join(OsString::from_vec(vec![0x80])))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EILSEQ)
    );
    assert_eq!(
        fs::metadata(mountpoint.join(".data"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EINVAL)
    );
    assert_eq!(
        fs::write(mountpoint.join("new.txt"), b"no")
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ENOSYS)
    );
    let writable = fs::OpenOptions::new()
        .write(true)
        .open(mountpoint.join("root.txt"))
        .expect("writable open");
    drop(writable);

    let mut stale = File::open(mountpoint.join("root.txt")).expect("open revision");
    retained
        .write("root.txt", b"revised")
        .await
        .expect("external revision");
    let mut buffer = Vec::new();
    assert_eq!(
        stale.read_to_end(&mut buffer).unwrap_err().raw_os_error(),
        Some(libc::ESTALE)
    );
    drop(stale);
    assert_eq!(
        fs::read(mountpoint.join("root.txt")).expect("new revision"),
        b"revised"
    );

    let mut passthrough = fs::OpenOptions::new()
        .write(true)
        .open(mountpoint.join("root.txt"))
        .expect("open before passthrough change");
    retained
        .set_residency("root.txt", Residency::Passthrough)
        .await
        .expect("passthrough residency");
    let passthrough_revision = retained.revision("root.txt").await.expect("revision");
    assert_eq!(
        passthrough.write_all(b"denied").unwrap_err().raw_os_error(),
        Some(libc::EROFS)
    );
    assert_eq!(
        retained
            .revision("root.txt")
            .await
            .expect("unchanged revision"),
        passthrough_revision
    );
    drop(passthrough);

    let unavailable = retained.entry("unavailable.txt").await.expect("metadata");
    let mut file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(mountpoint.join("unavailable.txt"))
        .expect("open unavailable");
    fs::remove_file(
        backing
            .join(".data")
            .join(unavailable.meta.file_id.to_string())
            .join(unavailable.meta.revision.to_string()),
    )
    .expect("remove retained revision");
    let mut buffer = Vec::new();
    assert_eq!(
        file.read_to_end(&mut buffer).unwrap_err().raw_os_error(),
        Some(libc::EIO)
    );
    assert_eq!(buffer, Vec::<u8>::new());
    file.seek(SeekFrom::Start(0))
        .expect("unavailable write offset");
    assert_eq!(
        file.write_all(b"x").unwrap_err().raw_os_error(),
        Some(libc::EIO)
    );
    assert_eq!(
        retained
            .revision("unavailable.txt")
            .await
            .expect("unavailable revision"),
        unavailable.meta.revision
    );
    drop(file);

    guard.unmount().await.expect("unmount");
    assert_eq!(
        fs::read_dir(&mountpoint).expect("empty mountpoint").count(),
        0
    );
    fs::remove_dir_all(backing).expect("backing cleanup");
    fs::remove_dir_all(mountpoint).expect("mount cleanup");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires /dev/fuse and unprivileged Linux mount support"]
async fn linux_mount_restart_persists_and_unmounts_cleanly() {
    let unique = Uuid::now_v7();
    let backing = env::temp_dir().join(format!("offs-fuse-restart-backing-{unique}"));
    let mountpoint = env::temp_dir().join(format!("offs-fuse-restart-mount-{unique}"));
    fs::create_dir(&mountpoint).expect("mountpoint");
    let file_system = FileSystem::open(&backing, 1_u8).await.expect("open");
    file_system
        .set_residency("", Residency::Full)
        .await
        .expect("full root");
    file_system
        .write("saved.txt", b"durable")
        .await
        .expect("write");

    let guard = mount(file_system.clone(), &mountpoint)
        .await
        .expect("first mount");
    assert_eq!(
        fs::read(mountpoint.join("saved.txt")).expect("first read"),
        b"durable"
    );
    let open_file = File::open(mountpoint.join("saved.txt")).expect("open file");
    drop(open_file);
    guard.unmount().await.expect("first unmount");
    drop(file_system);

    let reopened = FileSystem::open(&backing, 1_u8).await.expect("reopen");
    reopened
        .set_residency("", Residency::Full)
        .await
        .expect("restore full root");
    let guard = mount(reopened.clone(), &mountpoint)
        .await
        .expect("second mount");
    assert_eq!(
        fs::read(mountpoint.join("saved.txt")).expect("restart read"),
        b"durable"
    );
    guard.unmount().await.expect("second unmount");
    drop(reopened);

    assert_eq!(
        fs::read_dir(&mountpoint).expect("empty mountpoint").count(),
        0
    );
    fs::remove_dir_all(backing).expect("backing cleanup");
    fs::remove_dir_all(mountpoint).expect("mount cleanup");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires /dev/fuse and unprivileged Linux mount support"]
#[allow(clippy::too_many_lines, reason = "one real-mount acceptance scenario")]
async fn linux_mount_creates_writes_appends_syncs_and_restarts() {
    let unique = Uuid::now_v7();
    let backing = env::temp_dir().join(format!("offs-fuse-write-backing-{unique}"));
    let mountpoint = env::temp_dir().join(format!("offs-fuse-write-mount-{unique}"));
    fs::create_dir(&mountpoint).expect("mountpoint");
    let file_system = FileSystem::open(&backing, 1_u8).await.expect("open");
    file_system
        .set_residency("", Residency::Full)
        .await
        .expect("full root");
    file_system.create_dir("dir").await.expect("directory");
    let retained = file_system.clone();
    let guard = mount(file_system, &mountpoint).await.expect("mount");
    let path = mountpoint.join("created.txt");

    let mut stale = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)
        .expect("exclusive create");
    let created = retained.entry("created.txt").await.expect("created entry");
    assert_eq!(created.meta.mode, 0o644);
    assert_eq!(created.meta.kind, file_system::FileKind::File);
    assert_eq!(
        retained.read("created.txt").await.expect("empty create"),
        b""
    );
    assert_eq!(
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EEXIST)
    );
    assert_eq!(
        retained
            .entry("created.txt")
            .await
            .expect("duplicate entry")
            .meta,
        created.meta
    );

    let mut file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .expect("ordinary create opens existing file");
    file.write_all(b"abcdef").expect("initial write");
    file.sync_all().expect("fsync initial write");
    let written = retained.entry("created.txt").await.expect("written entry");
    assert_eq!(written.meta.file_id, created.meta.file_id);
    assert_ne!(written.meta.revision, created.meta.revision);
    assert_eq!(
        stale.write_all(b"stale").unwrap_err().raw_os_error(),
        Some(libc::ESTALE)
    );
    drop(stale);

    file.seek(SeekFrom::Start(2)).expect("overwrite seek");
    file.write_all(b"XY").expect("offset overwrite");
    file.seek(SeekFrom::Start(8)).expect("sparse seek");
    file.write_all(b"z").expect("sparse offset write");
    file.seek(SeekFrom::Start(0)).expect("start seek");
    file.write_all(b"Q").expect("start overwrite");
    file.flush().expect("flush writes");
    file.sync_data().expect("fdatasync writes");
    drop(file);
    let expected = b"QbXYef\0\0z";
    assert_eq!(fs::read(&path).expect("offset writes"), expected);

    let mut append = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .expect("append open");
    append
        .seek(SeekFrom::Start(0))
        .expect("ignored append seek");
    let mut conflicting_append = fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("conflicting append open");
    append.write_all(b"!").expect("append write");
    append.sync_all().expect("append fsync");
    assert_eq!(
        conflicting_append
            .write_all(b"stale")
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ESTALE)
    );
    drop(conflicting_append);
    drop(append);
    let expected = b"QbXYef\0\0z!";
    assert_eq!(fs::read(&path).expect("appended content"), expected);
    let appended = retained.entry("created.txt").await.expect("appended entry");
    assert_eq!(appended.meta.file_id, created.meta.file_id);

    assert_eq!(
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EEXIST)
    );
    assert_eq!(
        retained
            .entry("created.txt")
            .await
            .expect("final duplicate")
            .meta,
        appended.meta
    );
    assert_eq!(
        fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(mountpoint.join("missing/child"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ENOENT)
    );
    assert_eq!(
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(mountpoint.join("dir"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EEXIST)
    );
    assert_eq!(
        fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(mountpoint.join("dir"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EISDIR)
    );
    assert_eq!(
        fs::OpenOptions::new()
            .write(true)
            .open(mountpoint.join("absent"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ENOENT)
    );
    assert_eq!(
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(mountpoint.join(OsString::from_vec(vec![0x80])))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EILSEQ)
    );
    assert_eq!(
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(mountpoint.join(".data"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EINVAL)
    );

    guard.unmount().await.expect("first unmount");
    drop(retained);
    let reopened = FileSystem::open(&backing, 1_u8).await.expect("reopen");
    reopened
        .set_residency("", Residency::Full)
        .await
        .expect("restore full root");
    assert_eq!(
        reopened.read("created.txt").await.expect("durable bytes"),
        expected
    );
    assert_eq!(
        reopened
            .entry("created.txt")
            .await
            .expect("durable entry")
            .meta,
        appended.meta
    );
    let guard = mount(reopened.clone(), &mountpoint)
        .await
        .expect("second mount");
    assert_eq!(fs::read(&path).expect("restart mount read"), expected);
    guard.unmount().await.expect("second unmount");
    drop(reopened);
    fs::remove_dir_all(backing).expect("backing cleanup");
    fs::remove_dir_all(mountpoint).expect("mount cleanup");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires /dev/fuse and unprivileged Linux mount support"]
#[allow(clippy::too_many_lines, reason = "one real-mount acceptance scenario")]
async fn linux_mount_creates_directories_under_existing_parents() {
    let unique = Uuid::now_v7();
    let backing = env::temp_dir().join(format!("offs-fuse-mkdir-backing-{unique}"));
    let mountpoint = env::temp_dir().join(format!("offs-fuse-mkdir-mount-{unique}"));
    fs::create_dir(&mountpoint).expect("mountpoint");
    let file_system = FileSystem::open(&backing, 1_u8).await.expect("open");
    file_system
        .set_residency("", Residency::Full)
        .await
        .expect("full root");
    file_system.create_dir("parent").await.expect("parent");
    file_system.create_dir("denied").await.expect("denied");
    file_system.create_dir("stale").await.expect("stale");
    file_system
        .write("parent/file.txt", b"keep")
        .await
        .expect("existing file");
    let retained = file_system.clone();
    let guard = mount(file_system, &mountpoint).await.expect("mount");

    fs::create_dir(mountpoint.join("parent/nested")).expect("mkdir");
    let metadata = fs::metadata(mountpoint.join("parent/nested")).expect("mkdir attributes");
    assert!(metadata.is_dir());
    assert_eq!(metadata.permissions().mode() & 0o777, 0o755);
    let nested = retained.entry("parent/nested").await.expect("nested entry");
    assert_eq!(nested.meta.kind, file_system::FileKind::Directory);
    assert_eq!(nested.meta.mode, 0o755);
    assert_eq!(
        fs::create_dir(mountpoint.join("parent/nested"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EEXIST)
    );
    assert_eq!(
        fs::create_dir(mountpoint.join("parent/file.txt"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EEXIST)
    );
    assert_eq!(
        fs::create_dir(mountpoint.join("missing/child"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ENOENT)
    );
    assert_eq!(
        fs::create_dir(mountpoint.join("parent/file.txt/child"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ENOTDIR)
    );
    retained
        .delete("stale")
        .await
        .expect("external parent deletion");
    assert_eq!(
        fs::create_dir(mountpoint.join("stale/child"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ENOENT)
    );
    assert!(matches!(
        retained.entry("stale").await,
        Err(file_system::Error::NotFound)
    ));
    retained
        .set_residency("denied", Residency::Passthrough)
        .await
        .expect("passthrough residency");
    assert_eq!(
        fs::create_dir(mountpoint.join("denied/child"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EROFS)
    );
    assert_eq!(
        retained.list("denied").await.expect("denied entries"),
        [] as [file_system::Entry<u8>; 0]
    );
    retained
        .set_residency("denied", Residency::Full)
        .await
        .expect("restore full directory");
    assert_eq!(
        fs::create_dir(mountpoint.join(".data"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EINVAL)
    );
    assert_eq!(
        fs::create_dir(mountpoint.join(OsString::from_vec(vec![0x80])))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EILSEQ)
    );
    assert_eq!(
        retained
            .read("parent/file.txt")
            .await
            .expect("retained bytes"),
        b"keep"
    );
    assert_eq!(
        retained
            .list("parent")
            .await
            .expect("parent entries")
            .into_iter()
            .map(|entry| entry.path)
            .collect::<Vec<_>>(),
        ["parent/file.txt", "parent/nested"]
    );

    guard.unmount().await.expect("unmount");
    drop(retained);
    fs::remove_dir_all(backing).expect("backing cleanup");
    fs::remove_dir_all(mountpoint).expect("mount cleanup");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires /dev/fuse and unprivileged Linux mount support"]
#[allow(clippy::too_many_lines, reason = "one real-mount acceptance scenario")]
async fn linux_mount_unlinks_only_regular_files() {
    let unique = Uuid::now_v7();
    let backing = env::temp_dir().join(format!("offs-fuse-unlink-backing-{unique}"));
    let mountpoint = env::temp_dir().join(format!("offs-fuse-unlink-mount-{unique}"));
    fs::create_dir(&mountpoint).expect("mountpoint");
    let file_system = FileSystem::open(&backing, 1_u8).await.expect("open");
    file_system
        .set_residency("", Residency::Full)
        .await
        .expect("full root");
    file_system.create_dir("dir").await.expect("directory");
    file_system
        .write("dir/file.txt", b"delete")
        .await
        .expect("file to unlink");
    file_system.create_dir("occupied").await.expect("occupied");
    file_system
        .write("occupied/child.txt", b"keep")
        .await
        .expect("child to retain");
    file_system
        .write("denied.txt", b"denied")
        .await
        .expect("denied file");
    file_system
        .write("unavailable.txt", b"unavailable")
        .await
        .expect("unavailable file");
    let deleted = file_system
        .entry("dir/file.txt")
        .await
        .expect("delete entry");
    let unavailable = file_system
        .entry("unavailable.txt")
        .await
        .expect("unavailable entry");
    let denied = file_system.entry("denied.txt").await.expect("denied entry");
    let retained = file_system.clone();
    let guard = mount(file_system, &mountpoint).await.expect("mount");

    fs::remove_file(mountpoint.join("dir/file.txt")).expect("unlink");
    assert!(matches!(
        retained.entry("dir/file.txt").await,
        Err(file_system::Error::NotFound)
    ));
    assert!(
        backing
            .join(".data")
            .join(deleted.meta.file_id.to_string())
            .join(deleted.meta.revision.to_string())
            .is_file(),
        "unlink retains the prior revision through the existing deletion contract"
    );
    assert_eq!(
        directory_entries(&mountpoint.join("dir"), 0)
            .expect("iteration after unlink")
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>(),
        [".", ".."]
    );
    assert_eq!(
        fs::remove_file(mountpoint.join("occupied"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EISDIR)
    );
    assert_eq!(
        fs::remove_file(&mountpoint).unwrap_err().raw_os_error(),
        Some(libc::EISDIR)
    );
    assert_eq!(
        fs::remove_file(mountpoint.join("missing"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ENOENT)
    );
    assert_eq!(
        fs::remove_file(mountpoint.join("missing/child"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ENOENT)
    );
    assert_eq!(
        fs::remove_file(mountpoint.join("occupied/child.txt/child"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ENOTDIR)
    );
    assert_eq!(
        fs::remove_file(mountpoint.join(".data"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EINVAL)
    );
    assert_eq!(
        fs::remove_file(mountpoint.join(OsString::from_vec(vec![0x80])))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EILSEQ)
    );

    retained
        .set_residency("denied.txt", Residency::Passthrough)
        .await
        .expect("passthrough residency");
    let denied_content = backing
        .join(".data")
        .join(denied.meta.file_id.to_string())
        .join(denied.meta.revision.to_string());
    fs::create_dir_all(denied_content.parent().expect("revision directory"))
        .expect("retained revision directory");
    fs::write(&denied_content, b"denied").expect("retained passthrough revision");
    assert_eq!(
        fs::remove_file(mountpoint.join("denied.txt"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EROFS)
    );
    assert_eq!(
        retained
            .entry("denied.txt")
            .await
            .expect("denied preserved")
            .meta,
        denied.meta
    );
    assert!(denied_content.is_file());
    assert_eq!(
        retained
            .read("occupied/child.txt")
            .await
            .expect("child preserved"),
        b"keep"
    );

    fs::remove_file(
        backing
            .join(".data")
            .join(unavailable.meta.file_id.to_string())
            .join(unavailable.meta.revision.to_string()),
    )
    .expect("remove retained revision");
    assert_eq!(
        fs::remove_file(mountpoint.join("unavailable.txt"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EIO)
    );
    assert_eq!(
        retained
            .entry("unavailable.txt")
            .await
            .expect("unavailable preserved")
            .meta,
        unavailable.meta
    );

    guard.unmount().await.expect("unmount");
    drop(retained);
    let reopened = FileSystem::open(&backing, 1_u8).await.expect("reopen");
    assert!(matches!(
        reopened.entry("dir/file.txt").await,
        Err(file_system::Error::NotFound)
    ));
    assert_eq!(
        reopened
            .entry("denied.txt")
            .await
            .expect("denied restart")
            .meta,
        denied.meta
    );
    assert_eq!(
        reopened
            .read("occupied/child.txt")
            .await
            .expect("restart bytes"),
        b"keep"
    );
    drop(reopened);
    fs::remove_dir_all(backing).expect("backing cleanup");
    fs::remove_dir_all(mountpoint).expect("mount cleanup");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires /dev/fuse and unprivileged Linux mount support"]
#[allow(clippy::too_many_lines, reason = "one real-mount acceptance scenario")]
async fn linux_mount_removes_only_empty_directories_and_restarts() {
    let unique = Uuid::now_v7();
    let backing = env::temp_dir().join(format!("offs-fuse-rmdir-backing-{unique}"));
    let mountpoint = env::temp_dir().join(format!("offs-fuse-rmdir-mount-{unique}"));
    fs::create_dir(&mountpoint).expect("mountpoint");
    let file_system = FileSystem::open(&backing, 1_u8).await.expect("open");
    file_system
        .set_residency("", Residency::Full)
        .await
        .expect("full root");
    file_system.create_dir("empty").await.expect("empty");
    file_system
        .create_dir("tree/nested")
        .await
        .expect("nested directory");
    file_system
        .write("tree/file.txt", b"keep")
        .await
        .expect("child to retain");
    file_system
        .write("regular.txt", b"regular")
        .await
        .expect("regular file");
    file_system
        .create_dir("denied")
        .await
        .expect("denied directory");
    let child = file_system
        .entry("tree/file.txt")
        .await
        .expect("child entry");
    let nested = file_system
        .entry("tree/nested")
        .await
        .expect("nested entry");
    let retained = file_system.clone();
    let guard = mount(file_system, &mountpoint).await.expect("mount");

    assert_eq!(
        fs::remove_dir(mountpoint.join("tree"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ENOTEMPTY)
    );
    assert_eq!(
        retained.read("tree/file.txt").await.expect("child bytes"),
        b"keep"
    );
    assert_eq!(
        retained
            .entry("tree/file.txt")
            .await
            .expect("child preserved")
            .meta,
        child.meta
    );
    assert_eq!(
        retained
            .entry("tree/nested")
            .await
            .expect("nested preserved")
            .meta,
        nested.meta
    );
    assert_eq!(
        fs::remove_dir(mountpoint.join("regular.txt"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ENOTDIR)
    );
    assert_eq!(
        fs::remove_dir(&mountpoint).unwrap_err().raw_os_error(),
        Some(libc::EBUSY)
    );
    assert_eq!(
        fs::remove_dir(mountpoint.join("missing"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ENOENT)
    );
    assert_eq!(
        fs::remove_dir(mountpoint.join("missing/child"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ENOENT)
    );
    assert_eq!(
        fs::remove_dir(mountpoint.join("regular.txt/child"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ENOTDIR)
    );
    assert_eq!(
        fs::rename(
            mountpoint.join("regular.txt"),
            mountpoint.join("renamed.txt")
        )
        .unwrap_err()
        .raw_os_error(),
        Some(libc::ENOSYS)
    );
    assert_eq!(
        fs::remove_dir(mountpoint.join(".data"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EINVAL)
    );
    assert_eq!(
        fs::remove_dir(mountpoint.join(OsString::from_vec(vec![0x80])))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EILSEQ)
    );

    retained
        .set_residency("denied", Residency::Passthrough)
        .await
        .expect("passthrough residency");
    assert_eq!(
        fs::remove_dir(mountpoint.join("denied"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EROFS)
    );
    assert_eq!(
        retained
            .entry("denied")
            .await
            .expect("denied preserved")
            .meta
            .kind,
        file_system::FileKind::Directory
    );
    retained
        .set_residency("denied", Residency::Full)
        .await
        .expect("restore full directory");

    fs::remove_dir(mountpoint.join("empty")).expect("empty rmdir");
    assert!(matches!(
        retained.entry("empty").await,
        Err(file_system::Error::NotFound)
    ));
    fs::remove_file(mountpoint.join("tree/file.txt")).expect("unlink child");
    fs::remove_dir(mountpoint.join("tree/nested")).expect("nested rmdir");
    fs::remove_dir(mountpoint.join("tree")).expect("parent rmdir");
    assert_eq!(
        directory_entries(&mountpoint, 0)
            .expect("iteration after rmdir")
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>(),
        [".", "..", "denied", "regular.txt"]
    );
    assert_eq!(
        fs::read(mountpoint.join("regular.txt")).expect("remaining file"),
        b"regular"
    );

    guard.unmount().await.expect("first unmount");
    drop(retained);
    let reopened = FileSystem::open(&backing, 1_u8).await.expect("reopen");
    reopened
        .set_residency("", Residency::Full)
        .await
        .expect("restore full root");
    for path in ["empty", "tree", "tree/nested", "tree/file.txt"] {
        assert!(matches!(
            reopened.entry(path).await,
            Err(file_system::Error::NotFound)
        ));
    }
    assert_eq!(
        reopened.read("regular.txt").await.expect("restart bytes"),
        b"regular"
    );
    let guard = mount(reopened.clone(), &mountpoint)
        .await
        .expect("second mount");
    assert_eq!(
        directory_entries(&mountpoint, 0)
            .expect("restart iteration")
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>(),
        [".", "..", "denied", "regular.txt"]
    );
    guard.unmount().await.expect("second unmount");
    drop(reopened);
    fs::remove_dir_all(backing).expect("backing cleanup");
    fs::remove_dir_all(mountpoint).expect("mount cleanup");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires /dev/fuse and unprivileged Linux mount support"]
#[allow(clippy::too_many_lines, reason = "one real-mount acceptance scenario")]
async fn linux_mount_directory_handles_survive_namespace_mutations() {
    let unique = Uuid::now_v7();
    let backing = env::temp_dir().join(format!("offs-fuse-handles-backing-{unique}"));
    let mountpoint = env::temp_dir().join(format!("offs-fuse-handles-mount-{unique}"));
    fs::create_dir(&mountpoint).expect("mountpoint");
    let file_system = FileSystem::open(&backing, 1_u8).await.expect("open");
    file_system
        .set_residency("", Residency::Full)
        .await
        .expect("full root");
    file_system.create_dir("a").await.expect("directory a");
    file_system
        .write("a/old.txt", b"old")
        .await
        .expect("old file");
    file_system.create_dir("b").await.expect("directory b");
    file_system
        .write("b/keep.txt", b"keep")
        .await
        .expect("keep");
    let retained = file_system.clone();
    let guard = mount(file_system, &mountpoint).await.expect("mount");

    let held_root = Directory::open(&mountpoint).expect("held root handle");
    let held_a = Directory::open(&mountpoint.join("a")).expect("held directory handle");
    let root_before = held_root.entries(0).expect("root snapshot");
    let a_before = held_a.entries(0).expect("directory snapshot");
    assert_eq!(
        root_before
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        [".", "..", "a", "b"]
    );
    assert_eq!(
        a_before
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        [".", "..", "old.txt"]
    );
    let open_file = File::open(mountpoint.join("b/keep.txt")).expect("file handle");

    fs::create_dir(mountpoint.join("a/newdir")).expect("mkdir with open handles");
    fs::create_dir(mountpoint.join("c")).expect("root mkdir with open handles");
    fs::remove_file(mountpoint.join("a/old.txt")).expect("unlink with open handles");
    assert_eq!(
        held_root.entries(0).expect("stable root snapshot"),
        root_before
    );
    assert_eq!(
        held_a.entries(0).expect("stable directory snapshot"),
        a_before
    );

    let current_root = Directory::open(&mountpoint).expect("current root handle");
    let current_a = Directory::open(&mountpoint.join("a")).expect("current directory handle");
    let current_entries = current_root.entries(0).expect("current root entries");
    assert_eq!(
        current_entries
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        [".", "..", "a", "b", "c"]
    );
    assert_eq!(
        current_root
            .entries(current_entries[1].1)
            .expect("independent resumed offset")
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
    assert_eq!(
        current_a
            .entries(0)
            .expect("current directory entries")
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>(),
        [".", "..", "newdir"]
    );
    assert_eq!(
        held_a.entries(0).expect("held handle remains independent"),
        a_before
    );

    drop(open_file);
    drop(current_a);
    drop(current_root);
    drop(held_a);
    drop(held_root);
    guard.unmount().await.expect("unmount after releases");
    assert!(matches!(
        retained.entry("a/old.txt").await,
        Err(file_system::Error::NotFound)
    ));
    assert_eq!(
        retained
            .entry("a/newdir")
            .await
            .expect("new directory")
            .meta
            .kind,
        file_system::FileKind::Directory
    );
    assert_eq!(
        retained.read("b/keep.txt").await.expect("retained file"),
        b"keep"
    );
    drop(retained);
    fs::remove_dir_all(backing).expect("backing cleanup");
    fs::remove_dir_all(mountpoint).expect("mount cleanup");
}
