use std::{
    io,
    process::{Child, Command, ExitStatus, Stdio},
    sync::{Arc, Mutex, MutexGuard},
};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::os::unix::process::CommandExt;

use rustix::process::{Pid, Signal, kill_process_group};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use rustix::process::{WaitId, WaitIdOptions, waitid};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub(crate) struct Installation {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    cancelled: bool,
    child: Option<Pid>,
}

impl Installation {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(crate) fn cancel(&self) {
        let mut state = self.state();
        state.cancelled = true;
        if let Some(pid) = state.child {
            let _ = kill_process_group(pid, Signal::KILL);
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(crate) fn run(&self, command: &mut Command) -> bool {
        let mut state = self.state();
        if state.cancelled {
            return false;
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0);
        let child = match command.spawn() {
            Ok(child) => child,
            Err(_) => return false,
        };
        let pid = Pid::from_raw(child.id() as i32)
            .expect("spawned installer has a positive platform PID");
        state.child = Some(pid);
        drop(state);
        let mut owned = OwnedChild {
            child,
            pid,
            installation: self,
            finished: false,
        };
        let observed = loop {
            match waitid(
                WaitId::Pid(pid),
                WaitIdOptions::EXITED | WaitIdOptions::NOWAIT,
            ) {
                Ok(Some(_)) => break true,
                Ok(None) | Err(rustix::io::Errno::INTR) => continue,
                Err(_) => break false,
            }
        };
        owned
            .finish()
            .is_ok_and(|status| observed && status.success())
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    pub(crate) fn run(&self, _command: &mut Command) -> bool {
        false
    }
}

struct OwnedChild<'a> {
    child: Child,
    pid: Pid,
    installation: &'a Installation,
    finished: bool,
}

impl OwnedChild<'_> {
    fn finish(&mut self) -> io::Result<ExitStatus> {
        self.finished = true;
        let mut state = self.installation.state();
        // NOWAIT retains the leader PID until group termination, avoiding a reused-PID signal race.
        let _ = kill_process_group(self.pid, Signal::KILL);
        state.child = None;
        drop(state);
        self.child.wait()
    }
}

impl Drop for OwnedChild<'_> {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.finish();
        }
    }
}

pub(crate) struct CancelInstallation {
    installation: Arc<Installation>,
    cancellation: CancellationToken,
}

impl CancelInstallation {
    pub(crate) fn new(installation: Arc<Installation>, cancellation: CancellationToken) -> Self {
        Self {
            installation,
            cancellation,
        }
    }
}

impl Drop for CancelInstallation {
    fn drop(&mut self) {
        self.cancellation.cancel();
        self.installation.cancel();
    }
}
