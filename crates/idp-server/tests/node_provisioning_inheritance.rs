#![cfg(target_os = "linux")]

use std::{
    collections::HashMap,
    io::{self, Read, Write},
    os::unix::{net::UnixStream, process::CommandExt},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};

use idp_model::model::Id;
use idp_server::{AppConfig, OwnerProvisioner};
use idp_service::{
    oauth2::NodeSignerBinding,
    repo::{PrivateKeyRepo, RepoResult},
};
use key::{DerivationPath, DerivedKey};

#[derive(Clone, Default)]
struct Secrets {
    values: Arc<Mutex<HashMap<(String, String), String>>>,
    pause_store: Arc<Mutex<Option<StorePause>>>,
    store_calls: Arc<AtomicUsize>,
}

struct StorePause {
    entered: SyncSender<()>,
    resume: Receiver<()>,
}

impl PrivateKeyRepo for Secrets {
    fn load(&self, namespace: &str, path: &DerivationPath) -> RepoResult<Option<DerivedKey>> {
        Ok(self
            .values
            .lock()
            .expect("secret map")
            .get(&(namespace.into(), path.to_string()))
            .map(|value| DerivedKey::from_xprv(value.clone(), path.clone()))
            .transpose()?)
    }

    fn store(&self, namespace: &str, key: &DerivedKey) -> RepoResult<()> {
        self.store_calls.fetch_add(1, Ordering::SeqCst);
        let pause = self.pause_store.lock().expect("store pause").take();
        if let Some(pause) = pause {
            pause.entered.send(()).expect("store observer");
            pause
                .resume
                .recv_timeout(Duration::from_secs(30))
                .expect("release paused store");
        }
        self.values.lock().expect("secret map").insert(
            (namespace.into(), key.derivation_path().to_string()),
            key.to_xprv_string().to_string(),
        );
        Ok(())
    }

    fn delete(&self, namespace: &str, path: &DerivationPath) -> RepoResult<()> {
        self.values
            .lock()
            .expect("secret map")
            .remove(&(namespace.into(), path.to_string()));
        Ok(())
    }
}

#[tokio::test]
async fn completed_signer_operation_releases_lock_before_inheriting_child_exec() {
    let root = std::env::temp_dir().join(format!("inventory-inherited-lock-{}", Id::now_v7()));
    let config = AppConfig {
        data_dir: root.to_string_lossy().into_owned(),
        ..AppConfig::default()
    };
    let secrets = Secrets::default();
    let owner = Arc::new(
        OwnerProvisioner::open_with_private_keys(&config, secrets.clone())
            .await
            .expect("fresh owner"),
    );
    owner
        .prepare_fresh_setup_secret_inventory(Id::now_v7(), &[])
        .await
        .expect("fresh inventory");
    owner.prepare_setup_client_slots(&[]).await.expect("slots");
    let binding = NodeSignerBinding {
        member_id: Id::now_v7(),
        key_id: Id::now_v7(),
        endpoint_id: iroh::SecretKey::generate().public().to_string(),
    };
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (resume_tx, resume_rx) = mpsc::sync_channel(1);
    *secrets.pause_store.lock().expect("install store gate") = Some(StorePause {
        entered: entered_tx,
        resume: resume_rx,
    });
    let (mut gate, mut inherited_gate) = UnixStream::pair().expect("pre-exec gate");
    for stream in [&gate, &inherited_gate] {
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("bounded gate read");
        stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .expect("bounded gate write");
    }
    let mut command = Command::new(std::env::current_exe().expect("test executable"));
    command
        .arg("--list")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // Only stack buffers and async-signal-safe socket IO run after fork; timeouts are set above.
    unsafe {
        command.pre_exec(move || {
            if inherited_gate.write(&[1])? != 1 {
                return Err(io::ErrorKind::WriteZero.into());
            }
            let mut release = [0];
            if inherited_gate.read(&mut release)? != 1 || release != [2] {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            if inherited_gate.write(&[3])? != 1 {
                return Err(io::ErrorKind::WriteZero.into());
            }
            Ok(())
        });
    }
    let worker_owner = Arc::clone(&owner);
    let worker_binding = binding.clone();
    let worker = thread::spawn(move || worker_owner.prepare_node_signer(&worker_binding));
    entered_rx
        .recv_timeout(Duration::from_secs(30))
        .expect("real signer store entered with operation held");
    let child = thread::spawn(move || {
        let mut child = command.spawn()?;
        let deadline = Instant::now() + Duration::from_secs(10);
        let result = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
                Ok(None) => break Err(io::ErrorKind::TimedOut.into()),
                Err(error) => break Err(error),
            }
        };
        if result.is_err() {
            let _ = child.kill();
            let _ = child.wait();
        }
        result
    });

    let mut entered = [0];
    let inherited = gate.read_exact(&mut entered);
    let held = owner.verify_prepared_node_signer(&binding);
    let resumed = resume_tx.send(());
    let prepared = worker.join();
    let completed = owner.verify_prepared_node_signer(&binding);
    let mut acknowledged = [0];
    let released = gate
        .write_all(&[2])
        .and_then(|()| gate.read_exact(&mut acknowledged));
    let child = child.join();
    let after_exec = owner.verify_prepared_node_signer(&binding);
    drop(owner);
    std::fs::remove_dir_all(root).expect("remove owned fixture");

    inherited.expect("child inherited the held lock before exec");
    assert_eq!(entered, [1]);
    assert_eq!(
        held.expect_err("live operation must still exclude competitors")
            .kind(),
        io::ErrorKind::WouldBlock,
    );
    resumed.expect("resume real signer store");
    let public = prepared
        .expect("signer thread joined")
        .expect("signer prepared");
    released.expect("child stayed before exec until the verification completed");
    assert_eq!(acknowledged, [3]);
    assert!(
        child
            .expect("spawn thread joined")
            .expect("child reaped")
            .success()
    );
    assert_eq!(after_exec.expect("verification after child exec"), public);
    assert_eq!(secrets.store_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        completed.expect("completed operation must release exclusion before child exec"),
        public,
    );
}
