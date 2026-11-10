use std::{
    fs,
    net::SocketAddr,
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use rustix::process::{Pid, Signal, WaitOptions, kill_process, set_child_subreaper, waitpid};
use tokio_util::sync::CancellationToken;

use crate::IndependentSetupHost;

fn pid(root: &Path, name: &str) -> Option<Pid> {
    fs::read_to_string(root.join(name))
        .ok()?
        .trim()
        .parse::<i32>()
        .ok()
        .and_then(Pid::from_raw)
}

fn cleanup_scripts(root: &Path) {
    for name in ["descendant", "leader"] {
        if let Some(pid) = pid(root, name) {
            let _ = kill_process(pid, Signal::KILL);
        }
    }
}

#[test]
fn installation_cancellation_stops_processes_and_preserves_host() {
    installation_fixture("cancel");
}

#[test]
fn installation_parent_abort_stops_processes_and_preserves_host() {
    installation_fixture("abort");
}

#[test]
fn installation_future_drop_stops_processes_and_preserves_host() {
    installation_fixture("drop");
}

fn installation_fixture(mode: &str) {
    let root = tempfile::tempdir().expect("isolated installation fixture");
    let commands = root.path().join("commands");
    fs::create_dir(&commands).expect("isolated PATH");
    fs::write(
        commands.join("trust"),
        r#"#!/bin/sh
printf '%s\n' "$$" > "$INSTALL_TEST_ROOT/leader"
/bin/sleep 300 &
printf '%s\n' "$!" > "$INSTALL_TEST_ROOT/descendant"
wait
"#,
    )
    .expect("hanging installer with owned descendant");
    fs::set_permissions(commands.join("trust"), fs::Permissions::from_mode(0o700))
        .expect("test executable");
    let mut child = Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "setup_install_tests::installation_child",
            "--ignored",
            "--nocapture",
        ])
        .env("INSTALL_TEST_ROOT", root.path())
        .env("INSTALL_TEST_MODE", mode)
        .env("PATH", commands)
        .stdin(Stdio::null())
        .process_group(0)
        .spawn()
        .expect("isolated installation child");
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().expect("child status") {
            break status;
        }
        if Instant::now() >= deadline {
            cleanup_scripts(root.path());
            let _ = kill_process(
                Pid::from_raw(child.id() as i32).expect("child PID"),
                Signal::KILL,
            );
            child.wait().expect("reap bounded fixture");
            panic!("installation child exceeded existing 30-second fixture bound");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    cleanup_scripts(root.path());
    assert!(status.success(), "installation cancellation fixture");
}

fn prepare(root: &Path, address: SocketAddr) -> IndependentSetupHost {
    IndependentSetupHost::prepare(
        "idp",
        &root.join("idp"),
        address,
        "https://localhost:5173",
        "",
        false,
    )
    .expect("retained owner preparation")
    .expect("setup host")
}

#[tokio::test]
#[ignore = "parent supplies hanging PATH fixture; never runs real trust installation"]
async fn installation_child() {
    set_child_subreaper(Pid::from_raw(1)).expect("isolated child reaps its fixture descendants");
    let root = PathBuf::from(std::env::var_os("INSTALL_TEST_ROOT").expect("isolated root"));
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("reserve port");
    let address = listener.local_addr().expect("address");
    drop(listener);
    let host = prepare(&root, address);
    let state = fs::read(host.admission_path()).expect("retained admission");
    let ca = fs::read(host.tls().ca_path()).expect("retained CA");
    let mode = std::env::var("INSTALL_TEST_MODE").expect("fixture mode");
    let cancellation = CancellationToken::new();
    let mut serving = Some(Box::pin(host.serve(cancellation.clone())));
    let server = if mode == "drop" {
        None
    } else {
        Some(tokio::spawn(serving.take().expect("serving future")))
    };
    let started = tokio::time::timeout(Duration::from_secs(5), async {
        while pid(&root, "leader").is_none() || pid(&root, "descendant").is_none() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });
    if mode == "drop" {
        tokio::select! {
            result = serving.as_mut().expect("owned serving future") => panic!("setup exited before drop: {result:?}"),
            result = started => result.expect("installer and descendant started"),
        }
    } else {
        started.await.expect("installer and descendant started");
    }
    assert!(
        IndependentSetupHost::prepare(
            "idp",
            &root.join("idp"),
            address,
            "https://localhost:5173",
            "",
            false
        )
        .is_err(),
        "hung installation retains host ownership"
    );
    if mode == "drop" {
        drop(serving.take());
    } else {
        let server = server.expect("spawned serving future");
        if mode == "abort" {
            server.abort();
        } else {
            cancellation.cancel();
        }
        let stopped = tokio::time::timeout(Duration::from_secs(5), server).await;
        if stopped.is_err() {
            cleanup_scripts(&root);
        }
        let stopped = stopped.expect("cancellation must finish without selecting away");
        if mode == "abort" {
            assert!(stopped.expect_err("aborted parent").is_cancelled());
        } else {
            assert!(
                stopped.expect("parent task").is_err(),
                "cancelled setup cannot announce availability"
            );
        }
    }
    assert!(
        cancellation.is_cancelled(),
        "aborting parent establishes cancellation before installation"
    );
    let leader = pid(&root, "leader").expect("recorded leader");
    let descendant = pid(&root, "descendant").expect("recorded descendant");
    let reopened = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(Some(host)) = IndependentSetupHost::prepare(
                "idp",
                &root.join("idp"),
                address,
                "https://localhost:5173",
                "",
                false,
            ) {
                let descendant_stopped =
                    fs::read_to_string(format!("/proc/{}/stat", descendant.as_raw_pid()))
                        .map(|stat| stat.split_whitespace().nth(2) == Some("Z"))
                        .unwrap_or(true);
                assert!(
                    descendant_stopped,
                    "descendant must terminate before host/admission leases release"
                );
                assert!(
                    !Path::new(&format!("/proc/{}", leader.as_raw_pid())).exists(),
                    "worker must reap its direct child before leases release"
                );
                break host;
            }
            tokio::task::yield_now().await;
        }
    })
    .await;
    if reopened.is_err() {
        cleanup_scripts(&root);
    }
    let reopened = reopened.expect("cleanup releases retained host/admission guards");
    let descendant = pid(&root, "descendant").expect("recorded descendant");
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match waitpid(Some(descendant), WaitOptions::NOHANG) {
                Ok(Some(_)) | Err(rustix::io::Errno::CHILD) => break,
                Ok(None) | Err(rustix::io::Errno::INTR) => tokio::task::yield_now().await,
                Err(error) => panic!("fixture descendant reap failed: {error}"),
            }
        }
    })
    .await
    .expect("fixture reaps terminated descendant");
    let leader = pid(&root, "leader").expect("recorded leader");
    assert!(
        !Path::new(&format!("/proc/{}", leader.as_raw_pid())).exists(),
        "worker reaps its direct child"
    );
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("listener released");
    drop(listener);
    assert_eq!(
        fs::read(reopened.admission_path()).expect("unchanged admission"),
        state
    );
    assert_eq!(
        fs::read(reopened.tls().ca_path()).expect("unchanged TLS"),
        ca
    );
    assert!(!root.join("idp").exists(), "no domain or provider mutation");
}
