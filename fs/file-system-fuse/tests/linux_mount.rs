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
    time::SystemTime,
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

fn rename_with_flags(from: &Path, to: &Path, flags: u32) -> io::Result<()> {
    let from = CString::new(from.as_os_str().as_bytes())?;
    let to = CString::new(to.as_os_str().as_bytes())?;
    // SAFETY: both paths are terminated C strings that remain live for this call.
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            from.as_ptr(),
            libc::AT_FDCWD,
            to.as_ptr(),
            flags,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
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
    fs::write(mountpoint.join("new.txt"), b"no").expect("create with truncation");
    assert_eq!(
        fs::read(mountpoint.join("new.txt")).expect("created bytes"),
        b"no"
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

    let mut second_handle = fs::OpenOptions::new()
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
    second_handle
        .write_all(b"stale")
        .expect("second handle shares the admitted revision");
    drop(second_handle);

    file.seek(SeekFrom::Start(2)).expect("overwrite seek");
    file.write_all(b"XY").expect("offset overwrite");
    file.seek(SeekFrom::Start(8)).expect("sparse seek");
    file.write_all(b"z").expect("sparse offset write");
    file.seek(SeekFrom::Start(0)).expect("start seek");
    file.write_all(b"Q").expect("start overwrite");
    file.flush().expect("flush writes");
    file.sync_data().expect("fdatasync writes");
    drop(file);
    let expected = b"QtXYef\0\0z";
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
    conflicting_append
        .write_all(b"stale")
        .expect("second append uses the admitted end revision");
    drop(conflicting_append);
    drop(append);
    let expected = b"QtXYef\0\0z!stale";
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
async fn linux_mount_truncates_replaces_and_restarts() {
    let unique = Uuid::now_v7();
    let backing = env::temp_dir().join(format!("offs-fuse-truncate-backing-{unique}"));
    let mountpoint = env::temp_dir().join(format!("offs-fuse-truncate-mount-{unique}"));
    fs::create_dir(&mountpoint).expect("mountpoint");
    let file_system = FileSystem::open(&backing, 1_u8).await.expect("open");
    file_system
        .set_residency("", Residency::Full)
        .await
        .expect("full root");
    let original = file_system
        .write("file.txt", b"original contents")
        .await
        .expect("original file");
    let retained = file_system.clone();
    let guard = mount(file_system, &mountpoint).await.expect("mount");
    let path = mountpoint.join("file.txt");

    let mut file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .truncate(true)
        .open(&path)
        .expect("open with truncation");
    assert_eq!(fs::metadata(&path).expect("empty metadata").len(), 0);
    let opened = retained.entry("file.txt").await.expect("open truncation");
    assert_eq!(opened.meta.file_id, original.meta.file_id);
    assert_ne!(opened.meta.revision, original.meta.revision);

    file.write_all(b"replacement")
        .expect("write after open truncation");
    file.sync_all().expect("sync replacement");
    file.set_len(3).expect("descriptor shrink");
    assert_eq!(fs::read(&path).expect("shrunk bytes"), b"rep");
    file.seek(SeekFrom::Start(3)).expect("shrink write offset");
    file.write_all(b"!").expect("write after shrink");
    file.set_len(8).expect("descriptor extension");
    assert_eq!(fs::read(&path).expect("extended bytes"), b"rep!\0\0\0\0");
    file.seek(SeekFrom::Start(8))
        .expect("extension write offset");
    file.write_all(b"Z").expect("write after extension");
    assert_eq!(
        fs::read(&path).expect("post-extension write"),
        b"rep!\0\0\0\0Z"
    );

    file.set_len(0).expect("zero length");
    assert_eq!(fs::read(&path).expect("zero bytes"), b"");
    file.set_len(4).expect("extend empty file");
    file.seek(SeekFrom::Start(4))
        .expect("empty extension offset");
    file.write_all(b"X")
        .expect("write after extending empty file");
    assert_eq!(
        fs::read(&path).expect("zero filled empty extension"),
        b"\0\0\0\0X"
    );
    drop(file);

    let path_string = CString::new(path.as_os_str().as_bytes()).expect("path string");
    // SAFETY: `path_string` is terminated and remains live for the call.
    let result = unsafe { libc::truncate(path_string.as_ptr(), 2) };
    assert_eq!(result, 0, "path shrink: {}", io::Error::last_os_error());
    assert_eq!(fs::read(&path).expect("path-shrunk bytes"), b"\0\0");
    // SAFETY: `path_string` is terminated and remains live for the call.
    let result = unsafe { libc::truncate(path_string.as_ptr(), 6) };
    assert_eq!(result, 0, "path extension: {}", io::Error::last_os_error());
    assert_eq!(
        fs::read(&path).expect("path-extended bytes"),
        b"\0\0\0\0\0\0"
    );

    fs::write(&path, b"editor replacement").expect("editor-style truncate and replace");
    let expected = b"editor replacement";
    assert_eq!(fs::read(&path).expect("editor replacement bytes"), expected);
    let replaced = retained.entry("file.txt").await.expect("replacement entry");
    assert_eq!(replaced.meta.file_id, original.meta.file_id);
    fs::write(mountpoint.join("new.txt"), b"new file").expect("create with truncation flag");

    assert_eq!(
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ENOSYS)
    );
    assert_eq!(fs::read(&path).expect("chmod preserves bytes"), expected);
    assert_eq!(
        retained
            .revision("file.txt")
            .await
            .expect("chmod preserves revision"),
        replaced.meta.revision
    );
    let attributes = fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .expect("attribute handle");
    assert_eq!(
        attributes
            .set_times(
                fs::FileTimes::new()
                    .set_accessed(SystemTime::now())
                    .set_modified(SystemTime::now()),
            )
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ENOSYS)
    );
    assert_eq!(
        fs::read(&path).expect("timestamps preserve bytes"),
        expected
    );
    assert_eq!(
        retained
            .revision("file.txt")
            .await
            .expect("timestamps preserve revision"),
        replaced.meta.revision
    );
    drop(attributes);

    guard.unmount().await.expect("first unmount");
    drop(retained);
    let reopened = FileSystem::open(&backing, 1_u8).await.expect("reopen");
    reopened
        .set_residency("", Residency::Full)
        .await
        .expect("restore full root");
    assert_eq!(
        reopened.read("file.txt").await.expect("durable bytes"),
        expected
    );
    assert_eq!(
        reopened
            .entry("file.txt")
            .await
            .expect("durable entry")
            .meta,
        replaced.meta
    );
    let guard = mount(reopened.clone(), &mountpoint)
        .await
        .expect("second mount");
    assert_eq!(fs::read(&path).expect("restart mount read"), expected);
    assert_eq!(
        fs::read(mountpoint.join("new.txt")).expect("restart new file"),
        b"new file"
    );
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires /dev/fuse and unprivileged Linux mount support"]
#[allow(clippy::too_many_lines, reason = "one real-mount acceptance scenario")]
async fn linux_mount_rename_protects_open_handle_identities() {
    fn assert_read_write_error(file: &mut File, error: i32) {
        file.seek(SeekFrom::Start(0)).expect("rewind read");
        let mut bytes = Vec::new();
        assert_eq!(
            file.read_to_end(&mut bytes).unwrap_err().raw_os_error(),
            Some(error)
        );
        file.seek(SeekFrom::Start(0)).expect("rewind write");
        assert_eq!(
            file.write_all(b"forbidden").unwrap_err().raw_os_error(),
            Some(error)
        );
    }

    let unique = Uuid::now_v7();
    let backing = env::temp_dir().join(format!("offs-fuse-rename-handles-backing-{unique}"));
    let mountpoint = env::temp_dir().join(format!("offs-fuse-rename-handles-mount-{unique}"));
    fs::create_dir(&mountpoint).expect("mountpoint");
    let file_system = FileSystem::open(&backing, 1_u8).await.expect("open");
    file_system
        .set_residency("", Residency::Full)
        .await
        .expect("full root");
    for (path, content) in [
        ("mount-rename.txt", b"mount rename" as &[u8]),
        ("external-rename.txt", b"external rename"),
        ("delete.txt", b"delete"),
        ("recreate.txt", b"before recreate"),
        ("replace.txt", b"before replacement"),
        ("candidate.txt", b"replacement"),
    ] {
        file_system.write(path, content).await.expect("handle file");
    }
    let mount_before = file_system
        .entry("mount-rename.txt")
        .await
        .expect("mount rename entry");
    let external_before = file_system
        .entry("external-rename.txt")
        .await
        .expect("external rename entry");
    let replace_before = file_system
        .entry("replace.txt")
        .await
        .expect("replace entry");
    let candidate = file_system
        .entry("candidate.txt")
        .await
        .expect("candidate entry");
    let retained = file_system.clone();
    let guard = mount(file_system, &mountpoint).await.expect("mount");
    let open = |name: &str| {
        fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(mountpoint.join(name))
            .expect("open handle")
    };
    let mut mount_renamed = open("mount-rename.txt");
    let mut external_renamed = open("external-rename.txt");
    let mut deleted = open("delete.txt");
    let mut recreated = open("recreate.txt");
    let mut replaced = open("replace.txt");

    fs::rename(
        mountpoint.join("mount-rename.txt"),
        mountpoint.join("mount-moved.txt"),
    )
    .expect("rename with mount-open handle");
    assert_read_write_error(&mut mount_renamed, libc::ENOENT);
    assert_eq!(
        fs::read(mountpoint.join("mount-moved.txt")).expect("mount moved bytes"),
        b"mount rename"
    );
    assert_eq!(
        retained
            .entry("mount-moved.txt")
            .await
            .expect("mount moved identity")
            .meta
            .file_id,
        mount_before.meta.file_id
    );

    retained
        .rename("external-rename.txt", "external-moved.txt")
        .await
        .expect("external rename");
    assert_read_write_error(&mut external_renamed, libc::ENOENT);
    assert_eq!(
        retained
            .read("external-moved.txt")
            .await
            .expect("external moved bytes"),
        b"external rename"
    );
    retained
        .write("external-rename.txt", b"new entry")
        .await
        .expect("recreate renamed source");
    assert_read_write_error(&mut external_renamed, libc::ESTALE);
    assert_eq!(
        retained
            .read("external-rename.txt")
            .await
            .expect("recreated source bytes"),
        b"new entry"
    );
    assert_ne!(
        retained
            .entry("external-rename.txt")
            .await
            .expect("recreated source identity")
            .meta
            .file_id,
        external_before.meta.file_id
    );
    assert_eq!(
        retained
            .read("external-moved.txt")
            .await
            .expect("original moved bytes"),
        b"external rename"
    );

    retained
        .delete("delete.txt")
        .await
        .expect("external delete");
    assert_read_write_error(&mut deleted, libc::ENOENT);
    retained
        .delete("recreate.txt")
        .await
        .expect("delete before recreate");
    assert_read_write_error(&mut recreated, libc::ENOENT);
    retained
        .write("recreate.txt", b"after recreate")
        .await
        .expect("external recreate");
    assert_read_write_error(&mut recreated, libc::ESTALE);
    assert_eq!(
        retained
            .read("recreate.txt")
            .await
            .expect("recreated bytes"),
        b"after recreate"
    );

    retained
        .delete("replace.txt")
        .await
        .expect("delete before replacement");
    retained
        .rename("candidate.txt", "replace.txt")
        .await
        .expect("external replacement");
    assert_read_write_error(&mut replaced, libc::ESTALE);
    let replacement = retained
        .entry("replace.txt")
        .await
        .expect("replacement entry");
    assert_eq!(replacement.meta.file_id, candidate.meta.file_id);
    assert_ne!(replacement.meta.file_id, replace_before.meta.file_id);
    assert_eq!(
        retained
            .read("replace.txt")
            .await
            .expect("replacement bytes"),
        b"replacement"
    );

    drop(mount_renamed);
    drop(external_renamed);
    drop(deleted);
    drop(recreated);
    drop(replaced);
    guard.unmount().await.expect("unmount");
    drop(retained);
    fs::remove_dir_all(backing).expect("backing cleanup");
    fs::remove_dir_all(mountpoint).expect("mount cleanup");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires /dev/fuse and unprivileged Linux mount support"]
async fn linux_mount_renames_regular_files_without_replacement() {
    let unique = Uuid::now_v7();
    let backing = env::temp_dir().join(format!("offs-fuse-rename-file-backing-{unique}"));
    let mountpoint = env::temp_dir().join(format!("offs-fuse-rename-file-mount-{unique}"));
    fs::create_dir(&mountpoint).expect("mountpoint");
    let file_system = FileSystem::open(&backing, 1_u8).await.expect("open");
    file_system
        .set_residency("", Residency::Full)
        .await
        .expect("full root");
    file_system
        .create_dir("source")
        .await
        .expect("source parent");
    file_system
        .create_dir("destination")
        .await
        .expect("destination parent");
    let before = file_system
        .write("source/café 🦀.txt", b"stable bytes")
        .await
        .expect("source file");
    let retained = file_system.clone();
    let guard = mount(file_system, &mountpoint).await.expect("mount");

    fs::rename(
        mountpoint.join("source/café 🦀.txt"),
        mountpoint.join("destination/renamed 🦀.txt"),
    )
    .expect("no-replace file rename");

    assert_eq!(
        fs::read(mountpoint.join("destination/renamed 🦀.txt")).expect("renamed bytes"),
        b"stable bytes"
    );
    assert_eq!(
        fs::metadata(mountpoint.join("source/café 🦀.txt"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ENOENT)
    );
    let after = retained
        .entry("destination/renamed 🦀.txt")
        .await
        .expect("renamed entry");
    assert_eq!(after.meta.file_id, before.meta.file_id);
    assert_eq!(
        retained.read(&after.path).await.expect("core bytes"),
        b"stable bytes"
    );
    assert!(matches!(
        retained.entry("source/café 🦀.txt").await,
        Err(file_system::Error::NotFound)
    ));
    assert_eq!(
        directory_entries(&mountpoint.join("destination"), 0)
            .expect("destination iteration")
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>(),
        [".", "..", "renamed 🦀.txt"]
    );

    guard.unmount().await.expect("unmount");
    drop(retained);
    fs::remove_dir_all(backing).expect("backing cleanup");
    fs::remove_dir_all(mountpoint).expect("mount cleanup");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires /dev/fuse and unprivileged Linux mount support"]
#[allow(clippy::too_many_lines, reason = "one real-mount acceptance scenario")]
async fn linux_mount_renames_directory_trees_and_restarts() {
    let unique = Uuid::now_v7();
    let backing = env::temp_dir().join(format!("offs-fuse-rename-tree-backing-{unique}"));
    let mountpoint = env::temp_dir().join(format!("offs-fuse-rename-tree-mount-{unique}"));
    fs::create_dir(&mountpoint).expect("mountpoint");
    let file_system = FileSystem::open(&backing, 1_u8).await.expect("open");
    file_system
        .set_residency("", Residency::Full)
        .await
        .expect("full root");
    file_system.create_dir("target").await.expect("target");
    file_system
        .create_dir("tree/empty")
        .await
        .expect("empty child");
    file_system
        .create_dir("tree/nested/deep")
        .await
        .expect("nested directories");
    file_system
        .write("tree/file.txt", b"root child")
        .await
        .expect("root child");
    file_system
        .write("tree/nested/leaf.txt", b"nested child")
        .await
        .expect("nested child");
    file_system
        .create_dir("flagged-empty")
        .await
        .expect("flagged empty directory");
    let tracked = [
        ("tree", "target/moved"),
        ("tree/empty", "target/moved/empty"),
        ("tree/nested", "target/moved/nested"),
        ("tree/nested/deep", "target/moved/nested/deep"),
        ("tree/file.txt", "target/moved/file.txt"),
        ("tree/nested/leaf.txt", "target/moved/nested/leaf.txt"),
    ];
    let mut before = Vec::new();
    for (source, destination) in tracked {
        before.push((
            source,
            destination,
            file_system.entry(source).await.expect("tracked entry"),
        ));
    }
    let flagged = file_system
        .entry("flagged-empty")
        .await
        .expect("flagged entry");
    let retained = file_system.clone();
    let guard = mount(file_system, &mountpoint).await.expect("mount");

    rename_with_flags(
        &mountpoint.join("flagged-empty"),
        &mountpoint.join("target/flagged-empty"),
        libc::RENAME_NOREPLACE,
    )
    .expect("explicit no-replace rename");
    fs::rename(mountpoint.join("tree"), mountpoint.join("target/moved"))
        .expect("directory tree rename");

    for (source, destination, original) in &before {
        let moved = retained.entry(destination).await.expect("moved entry");
        assert_eq!(moved.meta.file_id, original.meta.file_id, "{source}");
        assert!(!moved.meta.revision.is_nil(), "{destination}");
        assert!(matches!(
            retained.entry(source).await,
            Err(file_system::Error::NotFound)
        ));
    }
    assert_eq!(
        retained
            .entry("target/flagged-empty")
            .await
            .expect("flagged moved entry")
            .meta
            .file_id,
        flagged.meta.file_id
    );
    assert!(
        fs::metadata(mountpoint.join("target/moved/empty"))
            .expect("empty child")
            .is_dir()
    );
    assert!(
        fs::metadata(mountpoint.join("target/moved/nested/deep"))
            .expect("deep child")
            .is_dir()
    );
    assert_eq!(
        fs::read(mountpoint.join("target/moved/file.txt")).expect("root child bytes"),
        b"root child"
    );
    assert_eq!(
        fs::read(mountpoint.join("target/moved/nested/leaf.txt")).expect("nested child bytes"),
        b"nested child"
    );
    assert_eq!(
        directory_entries(&mountpoint.join("target"), 0)
            .expect("target iteration")
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>(),
        [".", "..", "flagged-empty", "moved"]
    );

    guard.unmount().await.expect("first unmount");
    drop(retained);
    let reopened = FileSystem::open(&backing, 1_u8).await.expect("reopen");
    reopened
        .set_residency("", Residency::Full)
        .await
        .expect("restore full root");
    assert_eq!(
        reopened
            .read("target/moved/nested/leaf.txt")
            .await
            .expect("restart bytes"),
        b"nested child"
    );
    let guard = mount(reopened.clone(), &mountpoint)
        .await
        .expect("second mount");
    assert_eq!(
        fs::read(mountpoint.join("target/moved/file.txt")).expect("remount bytes"),
        b"root child"
    );
    assert_eq!(
        directory_entries(&mountpoint.join("target/moved"), 0)
            .expect("remount iteration")
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>(),
        [".", "..", "empty", "file.txt", "nested"]
    );
    guard.unmount().await.expect("second unmount");
    drop(reopened);
    fs::remove_dir_all(backing).expect("backing cleanup");
    fs::remove_dir_all(mountpoint).expect("mount cleanup");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires /dev/fuse and unprivileged Linux mount support"]
#[allow(clippy::too_many_lines, reason = "one real-mount acceptance scenario")]
async fn linux_mount_rename_failures_preserve_namespace() {
    let unique = Uuid::now_v7();
    let backing = env::temp_dir().join(format!("offs-fuse-rename-errors-backing-{unique}"));
    let mountpoint = env::temp_dir().join(format!("offs-fuse-rename-errors-mount-{unique}"));
    fs::create_dir(&mountpoint).expect("mountpoint");
    let file_system = FileSystem::open(&backing, 1_u8).await.expect("open");
    file_system
        .set_residency("", Residency::Full)
        .await
        .expect("full root");
    file_system
        .write("source.txt", b"source")
        .await
        .expect("source file");
    file_system
        .write("occupied.txt", b"occupied")
        .await
        .expect("occupied file");
    file_system
        .create_dir("tree/nested")
        .await
        .expect("source tree");
    file_system
        .write("tree/nested/child.txt", b"tree child")
        .await
        .expect("tree child");
    file_system
        .create_dir("occupied-dir")
        .await
        .expect("occupied directory");
    file_system
        .write("occupied-dir/keep.txt", b"keep")
        .await
        .expect("occupied child");
    file_system
        .create_dir("denied-source")
        .await
        .expect("denied source");
    file_system
        .create_dir("denied-destination")
        .await
        .expect("denied destination");
    file_system
        .write("flagged.txt", b"flagged")
        .await
        .expect("flagged source");
    let unavailable = file_system
        .write("unavailable.txt", b"unavailable")
        .await
        .expect("unavailable source");
    let source = file_system.entry("source.txt").await.expect("source entry");
    let tree = file_system.entry("tree").await.expect("tree entry");
    let tree_child = file_system
        .entry("tree/nested/child.txt")
        .await
        .expect("tree child entry");
    let retained = file_system.clone();
    let guard = mount(file_system, &mountpoint).await.expect("mount");

    for (source_name, destination_name, error) in [
        ("source.txt", "occupied.txt", libc::EEXIST),
        ("tree", "occupied-dir", libc::EEXIST),
        ("tree", "occupied.txt", libc::ENOTDIR),
        ("source.txt", "occupied-dir", libc::EISDIR),
    ] {
        assert_eq!(
            fs::rename(
                mountpoint.join(source_name),
                mountpoint.join(destination_name)
            )
            .unwrap_err()
            .raw_os_error(),
            Some(error),
            "{source_name} to {destination_name}"
        );
    }
    assert_eq!(
        rename_with_flags(
            &mountpoint.join("source.txt"),
            &mountpoint.join("occupied.txt"),
            libc::RENAME_NOREPLACE,
        )
        .unwrap_err()
        .raw_os_error(),
        Some(libc::EEXIST)
    );
    assert_eq!(
        fs::read(mountpoint.join("source.txt")).expect("source"),
        b"source"
    );
    assert_eq!(
        fs::read(mountpoint.join("occupied.txt")).expect("occupied"),
        b"occupied"
    );
    assert_eq!(
        fs::read(mountpoint.join("occupied-dir/keep.txt")).expect("occupied child"),
        b"keep"
    );

    rename_with_flags(
        &mountpoint.join("flagged.txt"),
        &mountpoint.join("flagged-moved.txt"),
        libc::RENAME_NOREPLACE,
    )
    .expect("flagged file rename");
    assert_eq!(
        fs::read(mountpoint.join("flagged-moved.txt")).expect("flagged bytes"),
        b"flagged"
    );

    // Linux FUSE maps a rename2 ENOSYS reply to EINVAL and disables later rename2 calls.
    for flags in [
        libc::RENAME_EXCHANGE,
        libc::RENAME_WHITEOUT,
        libc::RENAME_EXCHANGE | libc::RENAME_NOREPLACE,
    ] {
        assert_eq!(
            rename_with_flags(
                &mountpoint.join("source.txt"),
                &mountpoint.join("occupied.txt"),
                flags,
            )
            .unwrap_err()
            .raw_os_error(),
            Some(libc::EINVAL),
            "flags {flags}"
        );
    }
    assert_eq!(
        fs::rename(
            mountpoint.join("tree"),
            mountpoint.join("tree/nested/moved")
        )
        .unwrap_err()
        .raw_os_error(),
        Some(libc::EINVAL)
    );
    assert_eq!(
        retained
            .entry("tree/nested/child.txt")
            .await
            .expect("subtree child preserved")
            .meta,
        tree_child.meta
    );

    assert_eq!(
        fs::rename(mountpoint.join("missing.txt"), mountpoint.join("new.txt"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ENOENT)
    );
    assert_eq!(
        fs::rename(
            mountpoint.join("source.txt/child"),
            mountpoint.join("new.txt")
        )
        .unwrap_err()
        .raw_os_error(),
        Some(libc::ENOTDIR)
    );
    assert_eq!(
        fs::rename(
            mountpoint.join("source.txt"),
            mountpoint.join("source.txt/child")
        )
        .unwrap_err()
        .raw_os_error(),
        Some(libc::ENOTDIR)
    );
    assert_eq!(
        fs::rename(
            mountpoint.join("source.txt"),
            mountpoint.join("missing-parent/child")
        )
        .unwrap_err()
        .raw_os_error(),
        Some(libc::ENOENT)
    );
    assert!(matches!(
        retained.entry("missing-parent").await,
        Err(file_system::Error::NotFound)
    ));
    assert_eq!(
        fs::rename(mountpoint.join("source.txt"), mountpoint.join(".data"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EINVAL)
    );
    assert_eq!(
        fs::rename(
            mountpoint.join(OsString::from_vec(vec![0x80])),
            mountpoint.join("new.txt")
        )
        .unwrap_err()
        .raw_os_error(),
        Some(libc::EILSEQ)
    );
    assert_eq!(
        fs::rename(
            mountpoint.join("source.txt"),
            mountpoint.join(OsString::from_vec(vec![0x80]))
        )
        .unwrap_err()
        .raw_os_error(),
        Some(libc::EILSEQ)
    );

    retained
        .set_residency("denied-source", Residency::Passthrough)
        .await
        .expect("denied source residency");
    retained
        .set_residency("denied-destination", Residency::Passthrough)
        .await
        .expect("denied destination residency");
    assert_eq!(
        fs::rename(
            mountpoint.join("denied-source"),
            mountpoint.join("denied-moved")
        )
        .unwrap_err()
        .raw_os_error(),
        Some(libc::EROFS)
    );
    assert_eq!(
        fs::rename(
            mountpoint.join("source.txt"),
            mountpoint.join("denied-destination/moved")
        )
        .unwrap_err()
        .raw_os_error(),
        Some(libc::EROFS)
    );
    assert!(matches!(
        retained.entry("denied-moved").await,
        Err(file_system::Error::NotFound)
    ));
    assert!(matches!(
        retained.entry("denied-destination/moved").await,
        Err(file_system::Error::NotFound)
    ));

    fs::remove_file(
        backing
            .join(".data")
            .join(unavailable.meta.file_id.to_string())
            .join(unavailable.meta.revision.to_string()),
    )
    .expect("remove retained revision");
    assert_eq!(
        fs::rename(
            mountpoint.join("unavailable.txt"),
            mountpoint.join("unavailable-moved.txt")
        )
        .unwrap_err()
        .raw_os_error(),
        Some(libc::EIO)
    );
    assert_eq!(
        retained
            .entry("unavailable.txt")
            .await
            .expect("unavailable source preserved")
            .meta,
        unavailable.meta
    );
    assert!(matches!(
        retained.entry("unavailable-moved.txt").await,
        Err(file_system::Error::NotFound)
    ));

    assert_eq!(
        retained
            .entry("source.txt")
            .await
            .expect("source preserved")
            .meta,
        source.meta
    );
    assert_eq!(
        retained.entry("tree").await.expect("tree preserved").meta,
        tree.meta
    );
    assert!(matches!(
        retained.entry("new.txt").await,
        Err(file_system::Error::NotFound)
    ));

    guard.unmount().await.expect("unmount");
    drop(retained);
    fs::remove_dir_all(backing).expect("backing cleanup");
    fs::remove_dir_all(mountpoint).expect("mount cleanup");
}
