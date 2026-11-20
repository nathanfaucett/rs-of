use std::{
    io,
    path::{Path, PathBuf, absolute},
    sync::{Arc, Mutex},
};

use crate::resource::{file_identity, remove_owned_file, retired_error};

#[derive(Clone, Debug)]
pub struct OwnedFile {
    pub path: PathBuf,
    pub identity: String,
}

#[derive(Clone, Debug, Default)]
pub struct NativeStageTracker(Arc<Mutex<State>>);

#[derive(Debug, Default)]
struct State {
    retired: bool,
    files: Vec<OwnedFile>,
}

impl NativeStageTracker {
    /// Tracks a staged file and its identity.
    ///
    /// # Errors
    ///
    /// Returns an error when the stage is retired or path inspection fails.
    ///
    /// # Panics
    ///
    /// Panics if the stage tracker lock is poisoned.
    pub fn track(&self, path: &Path) -> io::Result<()> {
        let mut state = self.0.lock().expect("stage tracker lock");
        if state.retired {
            return Err(retired_error());
        }
        state.files.push(OwnedFile {
            path: absolute(path)?,
            identity: file_identity(path)?,
        });
        drop(state);
        Ok(())
    }

    /// Forgets a staged file if its absolute path can be resolved.
    ///
    /// # Panics
    ///
    /// Panics if the stage tracker lock is poisoned.
    pub fn forget(&self, path: &Path) {
        if let Ok(path) = absolute(path) {
            self.0
                .lock()
                .expect("stage tracker lock")
                .files
                .retain(|file| file.path != path);
        }
    }

    /// Marks the stage as retired and returns its owned files.
    ///
    /// # Panics
    ///
    /// Panics if the stage tracker lock is poisoned.
    #[must_use]
    pub fn retire(&self) -> Vec<OwnedFile> {
        let mut state = self.0.lock().expect("stage tracker lock");
        state.retired = true;
        state.files.clone()
    }

    /// Removes all tracked staged files.
    ///
    /// # Errors
    ///
    /// Returns an error when an owned file cannot be removed safely.
    ///
    /// # Panics
    ///
    /// Panics if the stage tracker lock is poisoned.
    pub fn cleanup(&self) -> io::Result<()> {
        let mut state = self.0.lock().expect("stage tracker lock");
        while let Some(file) = state.files.last() {
            remove_owned_file(file)?;
            state.files.pop();
        }
        drop(state);
        Ok(())
    }
}
