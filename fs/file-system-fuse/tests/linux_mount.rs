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

impl Drop for Directory {
    fn drop(&mut self) {
        // SAFETY: `opendir` returned this live pointer, and this owner closes it once.
        unsafe { libc::closedir(self.0) };
    }
}

fn directory_entries(path: &Path, offset: i64) -> io::Result<Vec<(String, i64)>> {
    let path = CString::new(path.as_os_str().as_bytes())?;
    // SAFETY: `path` is a terminated C string; the returned pointer is checked.
    let raw = unsafe { libc::opendir(path.as_ptr()) };
    if raw.is_null() {
        return Err(io::Error::last_os_error());
    }
    let directory = Directory(raw);
    if offset != 0 {
        // SAFETY: `directory` is live and `offset` came from a prior directory entry.
        unsafe { libc::seekdir(directory.0, offset) };
    }
    let mut entries = Vec::new();
    loop {
        // SAFETY: Linux exposes thread-local errno for this thread.
        unsafe { *libc::__errno_location() = 0 };
        // SAFETY: `directory` remains live until this loop finishes.
        let entry = unsafe { libc::readdir(directory.0) };
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
    assert_eq!(
        fs::remove_file(mountpoint.join("root.txt"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ENOSYS)
    );

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
