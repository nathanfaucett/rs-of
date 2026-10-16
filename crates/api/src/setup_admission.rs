use std::{
    fmt,
    fs::File,
    io::{Read, Write},
    os::unix::fs::MetadataExt,
    path::{Component, Path},
    sync::{Arc, Mutex},
};

use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    middleware::Next,
    response::{IntoResponse, Response},
};
use rustix::fs::{AtFlags, Mode, OFlags, mkdirat, open, openat, renameat, statat};
use subtle::ConstantTimeEq;

/// Host-local initial admission, not Installation identity, OAuth, peer enrollment,
/// or administrator authentication. Only wire this to setup routes.
///
/// The host must enforce HTTPS and origin/CORS restrictions. Explicit Authorization
/// avoids ambient-cookie/form CSRF, but is not an origin policy. Retrieve the code
/// from the private `state` file through protected host management only; never
/// return it over HTTP, log it, or put it in a URL. These Unix permissions protect
/// this capability, not existing domain private keys or their secure providers.
///
/// Methods perform synchronous durable IO; async hosts should use spawn_blocking.
/// Binding authorizes an exact retry, not mutual exclusion of provisioning work.
/// The host must coordinate that work and explicitly retire after completion;
/// dropping this handle or cancelling a request does neither.
pub struct SetupAdmission {
    inner: Mutex<LocalState>,
}

struct LocalState {
    directory: File,
    lock: File,
    failed: bool,
}

struct Retained {
    code: Option<String>,
    intent: String,
}

/// Deliberately contains no credential, intent, path, or underlying IO error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetupAdmissionError {
    Denied,
    Conflict,
    InvalidIntent,
    Unavailable,
}

impl fmt::Display for SetupAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Denied => "setup admission denied",
            Self::Conflict => "setup operation conflict",
            Self::InvalidIntent => "invalid setup operation identifier",
            Self::Unavailable => "setup admission unavailable",
        })
    }
}

impl std::error::Error for SetupAdmissionError {}

impl IntoResponse for SetupAdmissionError {
    fn into_response(self) -> Response {
        match self {
            Self::Denied => StatusCode::UNAUTHORIZED,
            Self::Conflict => StatusCode::CONFLICT,
            Self::InvalidIntent => StatusCode::BAD_REQUEST,
            Self::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        }
        .into_response()
    }
}

impl fmt::Debug for SetupAdmission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SetupAdmission { .. }")
    }
}

type Result<T> = std::result::Result<T, SetupAdmissionError>;

impl SetupAdmission {
    /// Automatically generate only when a dedicated root is absent in an existing
    /// private, euid-owned parent. An existing root must contain valid retained
    /// state and lock; missing/corrupt state is never regenerated or repaired.
    /// All path components must be nonsymlink directories. The host must establish
    /// that this admission root belongs to a genuinely fresh host, not use a new
    /// root to bypass lost admission or already-provisioned owner state.
    pub fn open(root: &Path) -> Result<Self> {
        let name = root.file_name().ok_or(SetupAdmissionError::Unavailable)?;
        let parent_path = root.parent().ok_or(SetupAdmissionError::Unavailable)?;
        let parent = open_directory(parent_path)?;
        private(&parent, true)?;
        let fresh = match mkdirat(&parent, name, Mode::from_raw_mode(0o700)) {
            Ok(()) => {
                parent.sync_all().map_err(unavailable)?;
                true
            }
            Err(rustix::io::Errno::EXIST) => false,
            Err(_) => return Err(SetupAdmissionError::Unavailable),
        };
        let directory: File = openat(&parent, name, directory_flags(), Mode::empty())
            .map_err(unavailable)?
            .into();
        private(&directory, true)?;
        let lock = local_file(&directory, "lock", fresh)?;
        lock.try_lock().map_err(unavailable)?;
        let mut local = LocalState {
            directory,
            lock,
            failed: false,
        };
        if fresh {
            local.lock.sync_all().map_err(unavailable)?;
            local.directory.sync_all().map_err(unavailable)?;
            let mut bytes = [0u8; 32];
            getrandom::fill(&mut bytes).map_err(unavailable)?;
            let code = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
            local.persist(&Retained {
                code: Some(code),
                intent: String::new(),
            })?;
        }
        local.read()?;
        Ok(Self {
            inner: Mutex::new(local),
        })
    }

    /// Validate private retained IO/state and sticky failure only. This is not
    /// admission proof: even valid retired state is healthy. Reveals no code or
    /// intent, and performs synchronous IO like the authorized operations.
    pub fn health(&self) -> Result<()> {
        let mut local = self.inner.lock().map_err(unavailable)?;
        if local.failed {
            return Err(SetupAdmissionError::Unavailable);
        }
        let result = local.read().map(|_| ());
        if result.is_err() {
            local.failed = true;
        }
        result
    }

    /// Check one explicit `Authorization: Setup <64 lowercase hex>` header.
    pub fn check(&self, headers: &HeaderMap) -> Result<()> {
        self.authorized(headers, |_, _| Ok(()))
    }

    /// Read the retained nonsecret intent with current setup authorization only.
    /// Does not bind, persist, or authorize a later request.
    pub fn bound_intent(&self, headers: &HeaderMap) -> Result<Option<String>> {
        let mut intent = None;
        self.authorized(headers, |_, retained| {
            if !retained.intent.is_empty() {
                intent = Some(retained.intent.clone());
            }
            Ok(())
        })?;
        Ok(intent)
    }

    /// Durably bind before any privileged mutation. The host supplies an exact,
    /// validated NONSECRET operation identifier (1..=1024 bytes, no controls).
    /// Never include raw passwords/secrets. This module cannot validate the host's
    /// operation contract or decide whether two different payloads mean a retry.
    pub fn bind(&self, headers: &HeaderMap, intent: &str) -> Result<()> {
        valid_intent(intent)?;
        self.authorized(headers, |local, retained| {
            if retained.intent.is_empty() {
                retained.intent = intent.to_owned();
                local.persist(retained)
            } else if retained.intent == intent {
                Ok(())
            } else {
                Err(SetupAdmissionError::Conflict)
            }
        })
    }

    /// Inspect durable finalization without granting setup authority or exposing its code.
    pub fn is_retired(&self) -> Result<bool> {
        let mut local = self.inner.lock().map_err(unavailable)?;
        if local.failed {
            return Err(SetupAdmissionError::Unavailable);
        }
        match local.read() {
            Ok(retained) => Ok(retained.code.is_none()),
            Err(error) => {
                local.failed = true;
                Err(error)
            }
        }
    }

    /// Read-only exact-result lookup after retirement; this grants no mutation authority.
    /// The host must additionally authenticate the completed User through its normal owner API.
    pub fn retired_matches(&self, intent: &str) -> Result<bool> {
        valid_intent(intent)?;
        let mut local = self.inner.lock().map_err(unavailable)?;
        if local.failed {
            return Err(SetupAdmissionError::Unavailable);
        }
        match local.read() {
            Ok(retained) => Ok(retained.code.is_none() && retained.intent == intent),
            Err(error) => {
                local.failed = true;
                Err(error)
            }
        }
    }

    /// Explicitly retire the bound operation after successful host completion.
    /// Retirement removes the retained code and survives restart.
    pub fn retire(&self, headers: &HeaderMap, intent: &str) -> Result<()> {
        valid_intent(intent)?;
        self.authorized(headers, |local, retained| {
            if retained.intent != intent {
                return Err(SetupAdmissionError::Conflict);
            }
            retained.code = None;
            local.persist(retained)
        })
    }

    fn authorized(
        &self,
        headers: &HeaderMap,
        operation: impl FnOnce(&mut LocalState, &mut Retained) -> Result<()>,
    ) -> Result<()> {
        let code = header_code(headers)?;
        let mut local = self.inner.lock().map_err(unavailable)?;
        if local.failed {
            return Err(SetupAdmissionError::Unavailable);
        }
        let result = (|| {
            let mut retained = local.read()?;
            let expected = retained.code.as_ref().ok_or(SetupAdmissionError::Denied)?;
            if !bool::from(expected.as_bytes().ct_eq(code.as_bytes())) {
                return Err(SetupAdmissionError::Denied);
            }
            operation(&mut local, &mut retained)
        })();
        if result == Err(SetupAdmissionError::Unavailable) {
            local.failed = true;
        }
        result
    }
}

/// Axum `from_fn_with_state(Arc<SetupAdmission>, setup_admission_middleware)`
/// for setup-only routes. This checks admission, not intent: handlers MUST call
/// `bind` with their validated operation identifier before privileged work.
pub async fn setup_admission_middleware(
    State(admission): State<Arc<SetupAdmission>>,
    request: Request,
    next: Next,
) -> Response {
    let headers = request.headers().clone();
    match tokio::task::spawn_blocking(move || admission.check(&headers)).await {
        Ok(Ok(())) => next.run(request).await,
        Ok(Err(error)) => error.into_response(),
        Err(_) => SetupAdmissionError::Unavailable.into_response(),
    }
}

impl LocalState {
    fn read(&self) -> Result<Retained> {
        private(&self.directory, true)?;
        let lock = local_file(&self.directory, "lock", false)?;
        let held = self.lock.metadata().map_err(unavailable)?;
        let current = lock.metadata().map_err(unavailable)?;
        if held.ino() != current.ino() || held.dev() != current.dev() {
            return Err(SetupAdmissionError::Unavailable);
        }
        match statat(&self.directory, "pending", AtFlags::SYMLINK_NOFOLLOW) {
            Err(rustix::io::Errno::NOENT) => (),
            _ => return Err(SetupAdmissionError::Unavailable),
        }
        let file = local_file(&self.directory, "state", false)?;
        let mut text = String::new();
        file.take(1200)
            .read_to_string(&mut text)
            .map_err(unavailable)?;
        let mut parts = text.splitn(3, '\n');
        if parts.next() != Some("setup-admission-v1") {
            return Err(SetupAdmissionError::Unavailable);
        }
        let code = parts.next().ok_or(SetupAdmissionError::Unavailable)?;
        let intent = parts
            .next()
            .ok_or(SetupAdmissionError::Unavailable)?
            .to_owned();
        if !intent.is_empty() && valid_intent(&intent).is_err() {
            return Err(SetupAdmissionError::Unavailable);
        }
        let code = if code == "retired" && !intent.is_empty() {
            None
        } else if valid_code(code) {
            Some(code.to_owned())
        } else {
            return Err(SetupAdmissionError::Unavailable);
        };
        Ok(Retained { code, intent })
    }

    fn persist(&mut self, retained: &Retained) -> Result<()> {
        let result = (|| {
            let mut file = local_file(&self.directory, "pending", true)?;
            write!(
                file,
                "setup-admission-v1\n{}\n{}",
                retained.code.as_deref().unwrap_or("retired"),
                retained.intent
            )
            .map_err(unavailable)?;
            file.sync_all().map_err(unavailable)?;
            renameat(&self.directory, "pending", &self.directory, "state").map_err(unavailable)?;
            self.directory.sync_all().map_err(unavailable)
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
}

fn header_code(headers: &HeaderMap) -> Result<&str> {
    let mut values = headers.get_all(AUTHORIZATION).iter();
    let value = values.next().ok_or(SetupAdmissionError::Denied)?;
    if values.next().is_some() {
        return Err(SetupAdmissionError::Denied);
    }
    let code = value
        .to_str()
        .ok()
        .and_then(|value| value.strip_prefix("Setup "))
        .filter(|code| valid_code(code))
        .ok_or(SetupAdmissionError::Denied)?;
    Ok(code)
}

fn valid_code(code: &str) -> bool {
    code.len() == 64
        && code
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_intent(intent: &str) -> Result<()> {
    if intent.is_empty() || intent.len() > 1024 || intent.chars().any(char::is_control) {
        Err(SetupAdmissionError::InvalidIntent)
    } else {
        Ok(())
    }
}

fn private(file: &File, directory: bool) -> Result<()> {
    let metadata = file.metadata().map_err(unavailable)?;
    if metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o077 != 0
        || if directory {
            !metadata.is_dir()
        } else {
            !metadata.is_file() || metadata.nlink() != 1 || metadata.len() > 1190
        }
    {
        return Err(SetupAdmissionError::Unavailable);
    }
    Ok(())
}

fn local_file(directory: &File, name: &str, fresh: bool) -> Result<File> {
    let flags = OFlags::RDWR
        | OFlags::NOFOLLOW
        | OFlags::CLOEXEC
        | OFlags::NONBLOCK
        | if fresh {
            OFlags::CREATE | OFlags::EXCL
        } else {
            OFlags::empty()
        };
    let file: File = openat(directory, name, flags, Mode::from_raw_mode(0o600))
        .map_err(unavailable)?
        .into();
    private(&file, false)?;
    Ok(file)
}

fn directory_flags() -> OFlags {
    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC
}

fn open_directory(path: &Path) -> Result<File> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir().map_err(unavailable)?.join(path)
    };
    let mut directory: File = open("/", directory_flags(), Mode::empty())
        .map_err(unavailable)?
        .into();
    for component in absolute.components() {
        match component {
            Component::RootDir | Component::CurDir => (),
            Component::Normal(name) => {
                directory = openat(&directory, name, directory_flags(), Mode::empty())
                    .map_err(unavailable)?
                    .into();
            }
            _ => return Err(SetupAdmissionError::Unavailable),
        }
    }
    Ok(directory)
}

fn unavailable<T>(_: T) -> SetupAdmissionError {
    SetupAdmissionError::Unavailable
}
