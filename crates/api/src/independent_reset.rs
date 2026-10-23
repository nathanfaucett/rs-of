use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use rustls::pki_types::{CertificateDer, pem::PemObject};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    IndependentCreateIntent, SetupAdmission, SetupScope,
    independent_setup::HostSettings,
    setup_files::{
        absolute, present, private_options, read_private, require_private, sync_ancestry,
    },
};

#[derive(Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Journal {
    version: u8,
    owner: String,
    domain: PathBuf,
    host: Value,
    intent: Option<IndependentCreateIntent>,
    worker: Option<Value>,
    initialized: bool,
    ca_fingerprint: String,
    phase: Phase,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum Phase {
    Secrets,
    Trust,
    Data,
    Cleanup,
}

/// Stopped, owner-local host device only. No HTTP reset capability is exposed.
/// Keep this guard alive through provider cleanup and local data removal.
pub struct IndependentReset {
    journal: Journal,
    directory: PathBuf,
    retired: PathBuf,
    journal_path: PathBuf,
    _operation: Option<File>,
}

impl IndependentReset {
    pub fn open(owner: &str, domain: &Path, remove_os_trust: bool) -> io::Result<Self> {
        if !remove_os_trust {
            return Err(io::Error::other(
                "reset requires explicit --remove-os-trust; local CA evidence must remain until OS trust removal succeeds",
            ));
        }
        SetupScope::from_owners(&[owner])?;
        let domain = absolute(domain)?;
        let directory = domain.with_extension("setup-host");
        let retired = domain.with_extension("setup-reset");
        let journal_path = domain.with_extension("setup-reset.json");
        for path in [&directory, &retired, &journal_path] {
            if path == &domain || path.starts_with(&domain) || domain.starts_with(path) {
                return Err(io::Error::other("reset roots must be disjoint"));
            }
        }
        let retained: Option<Journal> = if present(&journal_path)? {
            Some(serde_json::from_slice(&read_private(&journal_path)?)?)
        } else {
            None
        };
        if let Some(journal) = &retained
            && (journal.version != 1 || journal.owner != owner || journal.domain != domain)
        {
            return Err(io::Error::other(
                "reset inventory conflicts with selected owner configuration",
            ));
        }
        let cleanup = retained.as_ref().is_some_and(|j| j.phase == Phase::Cleanup);
        let retired_exists = present(&retired)?;
        let live_exists = present(&directory)?;
        if retired_exists && (!cleanup || live_exists) {
            return Err(io::Error::other(
                "retired reset evidence conflicts with live setup; preserve both roots",
            ));
        }
        let lock_root = if retired_exists { &retired } else { &directory };
        let operation = if cleanup && !live_exists && !retired_exists {
            None
        } else {
            require_private(lock_root, true)?;
            let lock_path = lock_root.join("operation.lock");
            let missing = !present(&lock_path)?;
            if missing && !retired_exists {
                return Err(io::Error::other(
                    "missing setup operation lock; exact recovery required",
                ));
            }
            if !missing {
                require_private(&lock_path, false)?;
            }
            let file = private_options(missing).open(lock_path)?;
            file.try_lock().map_err(|error| match error {
                fs::TryLockError::WouldBlock => io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "stop the independent owner host before reset (operation.lock is active)",
                ),
                fs::TryLockError::Error(error) => error,
            })?;
            Some(file)
        };
        let journal = match retained {
            Some(journal) => {
                if serde_json::from_slice::<Journal>(&read_private(&journal_path)?)? != journal {
                    return Err(io::Error::other(
                        "reset inventory changed while acquiring stopped-worker lock",
                    ));
                }
                if !cleanup && Self::capture(owner, &domain, &directory, &journal.phase)? != journal
                {
                    return Err(io::Error::other(
                        "retained setup changed since reset was requested",
                    ));
                }
                journal
            }
            None => Self::capture(owner, &domain, &directory, &Phase::Secrets)?,
        };
        let reset = Self {
            journal,
            directory,
            retired,
            journal_path,
            _operation: operation,
        };
        if !present(&reset.journal_path)? {
            reset.save()?;
        }
        Ok(reset)
    }

    fn capture(owner: &str, domain: &Path, directory: &Path, phase: &Phase) -> io::Result<Journal> {
        let host: Value = serde_json::from_slice(&read_private(&directory.join("host.json"))?)?;
        let settings: HostSettings = serde_json::from_value(host.clone())?;
        if host["version"] != 1 || settings.owner != owner || settings.domain != domain {
            return Err(io::Error::other(
                "retained setup does not own selected domain root",
            ));
        }
        require_private(&directory.join("setup-admission"), true)?;
        let admission =
            SetupAdmission::open(&directory.join("setup-admission")).map_err(io::Error::other)?;
        let intent: Option<IndependentCreateIntent> = admission
            .host_intent()
            .map_err(io::Error::other)?
            .map(|value| serde_json::from_str(&value))
            .transpose()?;
        if let Some(intent) = &intent {
            intent.canonical()?;
        }
        let initialized = present(&directory.join("initialized.json"))?;
        if initialized && read_private(&directory.join("initialized.json"))? != b"true" {
            return Err(io::Error::other("invalid initialized setup fence"));
        }
        let worker = if present(&directory.join("worker.json"))? {
            Some(serde_json::from_slice(&read_private(
                &directory.join("worker.json"),
            )?)?)
        } else {
            None
        };
        if (initialized && worker.is_none())
            || (worker.is_some() && (!initialized || intent.is_none()))
            || (present(domain)? && worker.is_none())
        {
            return Err(io::Error::other(
                "missing exact worker/fence evidence; reset cannot infer protected references",
            ));
        }
        Ok(Journal {
            version: 1,
            owner: owner.into(),
            domain: domain.into(),
            host,
            intent,
            worker,
            initialized,
            ca_fingerprint: ca_fingerprint(&directory.join("tls"))?,
            phase: *phase,
        })
    }

    fn save(&self) -> io::Result<()> {
        let temporary = self.journal_path.with_extension(format!(
            "json.{}",
            crate::independent_exchange::random_capability()?
        ));
        let mut file = private_options(true).open(&temporary)?;
        file.write_all(&serde_json::to_vec(&self.journal)?)?;
        file.sync_all()?;
        fs::rename(&temporary, &self.journal_path)?;
        sync_ancestry(
            self.journal_path
                .parent()
                .expect("absolute reset journal parent"),
        )
    }

    pub fn read_domain_document(&self, name: &str) -> io::Result<Value> {
        let mut components = Path::new(name).components();
        if !matches!(components.next(), Some(std::path::Component::Normal(_)))
            || components.next().is_some()
        {
            return Err(io::Error::other(
                "reset document must be an owner-local filename",
            ));
        }
        serde_json::from_slice(&read_private(&self.journal.domain.join(name))?)
            .map_err(io::Error::other)
    }

    pub fn worker(&self) -> Option<&Value> {
        self.journal.worker.as_ref()
    }
    pub fn intent(&self) -> Option<&IndependentCreateIntent> {
        self.journal.intent.as_ref()
    }
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub fn secrets_pending(&self) -> bool {
        self.journal.phase == Phase::Secrets
    }

    /// The owner must delete and verify its exact protected inventory before advancing.
    pub fn secrets_removed(&mut self) -> io::Result<()> {
        if self.secrets_pending() {
            self.journal.phase = Phase::Trust;
            self.save()?;
        }
        Ok(())
    }

    pub fn finish(self) -> io::Result<()> {
        self.finish_with_trust(uninstall_local_ca)
    }

    fn finish_with_trust(mut self, remove: impl FnOnce(&Path) -> io::Result<()>) -> io::Result<()> {
        if self.secrets_pending() {
            return Err(io::Error::other(
                "protected secret deletion has not completed",
            ));
        }
        if self.journal.phase == Phase::Trust {
            remove(&self.directory.join("tls"))?;
            self.journal.phase = Phase::Data;
            self.save()?;
        }
        if self.journal.phase == Phase::Data {
            if present(&self.journal.domain)? {
                fs::remove_dir_all(&self.journal.domain)?;
            }
            self.journal.phase = Phase::Cleanup;
            self.save()?;
        }
        if present(&self.directory)? {
            if present(&self.retired)?
                || Self::capture(
                    &self.journal.owner,
                    &self.journal.domain,
                    &self.directory,
                    &Phase::Cleanup,
                )? != self.journal
            {
                return Err(io::Error::other(
                    "setup cleanup inventory changed; evidence retained",
                ));
            }
            fs::rename(&self.directory, &self.retired)?;
            sync_ancestry(self.retired.parent().expect("absolute retired root parent"))?;
        }
        if present(&self.retired)? {
            fs::remove_dir_all(&self.retired)?;
        }
        fs::remove_file(&self.journal_path)?;
        sync_ancestry(
            self.journal_path
                .parent()
                .expect("absolute reset journal parent"),
        )
    }
}

fn ca_fingerprint(directory: &Path) -> io::Result<String> {
    let pem = read_private(&directory.join("ca.pem"))?;
    let der = read_private(&directory.join("ca.der"))?;
    let certificate = CertificateDer::from_pem_slice(&pem).map_err(io::Error::other)?;
    if certificate.as_ref() != der {
        return Err(io::Error::other(
            "retained CA encodings disagree; preserve trust evidence",
        ));
    }
    let hash = rustls::crypto::ring::cipher_suite::TLS13_AES_128_GCM_SHA256
        .tls13()
        .expect("TLS 1.3 suite")
        .common
        .hash_provider
        .hash(&der);
    Ok(hash.as_ref().iter().map(|b| format!("{b:02x}")).collect())
}

/// Explicit local operation. Native authorization/elevation is delegated to platform tools.
pub fn uninstall_local_ca(directory: &Path) -> io::Result<()> {
    uninstall_local_ca_with(directory, |command| {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
    })
}

fn uninstall_local_ca_with(
    directory: &Path,
    mut run: impl FnMut(&mut Command) -> io::Result<bool>,
) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    let pem = directory.join("ca.pem");
    let fingerprint = ca_fingerprint(directory)?;
    #[cfg(target_os = "linux")]
    {
        return uninstall_linux_ca(
            directory,
            &fingerprint,
            &[
                (
                    Path::new("/usr/local/share/ca-certificates"),
                    &["update-ca-certificates"],
                ),
                (
                    Path::new("/etc/pki/ca-trust/source/anchors"),
                    &["update-ca-trust", "extract"],
                ),
            ],
            &mut run,
        );
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME").ok_or_else(|| io::Error::other("HOME is missing"))?;
        let keychain = PathBuf::from(home).join("Library/Keychains/login.keychain-db");
        if run(Command::new("security")
            .arg("remove-trusted-cert")
            .arg(&pem))?
            && run(Command::new("security")
                .args(["delete-certificate", "-Z", &fingerprint])
                .arg(keychain))?
        {
            return Ok(());
        }
    }
    #[allow(unreachable_code)]
    Err(io::Error::other(
        "OS CA removal unavailable or denied; local TLS evidence retained",
    ))
}

#[cfg(target_os = "linux")]
fn uninstall_linux_ca(
    directory: &Path,
    fingerprint: &str,
    stores: &[(&Path, &[&str])],
    run: &mut impl FnMut(&mut Command) -> io::Result<bool>,
) -> io::Result<()> {
    let pem = directory.join("ca.pem");
    match run(Command::new("trust").args(["anchor", "--remove"]).arg(&pem)) {
        Ok(true) => (),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if !stores.iter().any(|(root, _)| root.is_dir()) {
                return Err(io::Error::other(
                    "no supported OS trust removal mechanism; retain TLS evidence",
                ));
            }
        }
        Ok(false) => {
            if !run(Command::new("pkexec")
                .args(["trust", "anchor", "--remove"])
                .arg(&pem))?
            {
                return Err(io::Error::other(
                    "OS trust removal failed; approve native consent and retry reset",
                ));
            }
        }
        Err(error) => return Err(error),
    }
    for (root, refresh) in stores {
        let anchor = root.join(format!("unified-node-{fingerprint}.crt"));
        if present(&anchor)? {
            let installed =
                CertificateDer::from_pem_slice(&fs::read(&anchor)?).map_err(io::Error::other)?;
            if installed.as_ref() != read_private(&directory.join("ca.der"))? {
                return Err(io::Error::other(
                    "distro anchor contents disagree; unrelated certificate preserved",
                ));
            }
            if !run(Command::new("pkexec").arg("rm").arg("--").arg(&anchor))? || present(&anchor)? {
                return Err(io::Error::other(
                    "distro CA removal failed; retain TLS evidence and retry reset",
                ));
            }
        }
        // A prior deletion may have succeeded before refresh failed.
        if root.is_dir() && !run(Command::new("pkexec").args(*refresh))? {
            return Err(io::Error::other(
                "distro trust refresh failed; retain TLS evidence and retry reset",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{fs, io, os::unix::fs::PermissionsExt};

    use tempfile::TempDir;

    use super::IndependentReset;
    use crate::IndependentSetupHost;

    #[cfg(target_os = "linux")]
    #[test]
    fn trust_removal_invokes_only_exact_retained_ca_with_native_consent_and_preserves_on_failure() {
        let root = TempDir::new().expect("isolated root");
        let domain = root.path().join("storage");
        let host = IndependentSetupHost::prepare(
            "storage",
            &domain,
            "127.0.0.1:31996".parse().expect("address"),
            "https://localhost:5173",
            "/storage",
            false,
        )
        .expect("prepare")
        .expect("host");
        let ca = host.tls().ca_path();
        let bytes = fs::read(&ca).expect("isolated CA");
        let mut calls = Vec::new();
        assert!(
            super::uninstall_local_ca_with(ca.parent().expect("TLS directory"), |command| {
                calls.push((
                    command.get_program().to_owned(),
                    command
                        .get_args()
                        .map(ToOwned::to_owned)
                        .collect::<Vec<_>>(),
                ));
                Ok(false)
            })
            .is_err()
        );
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, "trust");
        assert_eq!(calls[1].0, "pkexec");
        assert_eq!(calls[0].1.last(), Some(&ca.into_os_string()));
        assert_eq!(fs::read(host.tls().ca_path()).expect("retained CA"), bytes);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn distro_only_trust_removal_is_exact_and_retains_failure_evidence() {
        use std::ffi::OsStr;
        let root = TempDir::new().expect("isolated root");
        let domain = root.path().join("storage");
        let host = IndependentSetupHost::prepare(
            "storage",
            &domain,
            "127.0.0.1:31993".parse().expect("address"),
            "https://localhost:5173",
            "/storage",
            false,
        )
        .expect("prepare")
        .expect("host");
        let tls = host
            .tls()
            .ca_path()
            .parent()
            .expect("TLS directory")
            .to_owned();
        let fingerprint = super::ca_fingerprint(&tls).expect("fingerprint");
        let store = root.path().join("anchors");
        fs::create_dir(&store).expect("isolated store");
        let anchor = store.join(format!("unified-node-{fingerprint}.crt"));
        let unrelated = store.join("unrelated.crt");
        fs::write(&unrelated, b"unrelated").expect("unrelated certificate");
        let roots = [(store.as_path(), &["isolated-refresh"][..])];
        for failure in ["remove", "refresh", "none"] {
            fs::copy(tls.join("ca.pem"), &anchor).expect("owned anchor");
            let result = super::uninstall_linux_ca(&tls, &fingerprint, &roots, &mut |command| {
                if command.get_program() == "trust" {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        "no trust executable",
                    ));
                }
                assert_eq!(command.get_program(), "pkexec");
                let args: Vec<_> = command.get_args().collect();
                if args[0] == "rm" {
                    assert_eq!(
                        args,
                        [OsStr::new("rm"), OsStr::new("--"), anchor.as_os_str()]
                    );
                    if failure == "remove" {
                        return Ok(false);
                    }
                    fs::remove_file(&anchor).expect("injected exact removal");
                } else {
                    assert_eq!(args, [OsStr::new("isolated-refresh")]);
                    if failure == "refresh" {
                        return Ok(false);
                    }
                }
                Ok(true)
            });
            assert_eq!(result.is_ok(), failure == "none");
            assert_eq!(anchor.exists(), failure == "remove");
            assert!(tls.join("ca.pem").exists(), "reset evidence preserved");
            assert_eq!(
                fs::read(&unrelated).expect("unrelated preserved"),
                b"unrelated"
            );
            if failure == "refresh" {
                let mut refreshed = false;
                super::uninstall_linux_ca(&tls, &fingerprint, &roots, &mut |command| {
                    if command.get_program() == "trust" {
                        return Err(io::Error::new(io::ErrorKind::NotFound, "absent"));
                    }
                    assert_eq!(
                        command.get_args().collect::<Vec<_>>(),
                        [OsStr::new("isolated-refresh")]
                    );
                    refreshed = true;
                    Ok(true)
                })
                .expect("retry refresh after successful deletion");
                assert!(refreshed);
            }
        }
        fs::write(&anchor, fs::read(&unrelated).expect("unrelated")).expect("wrong contents");
        assert!(
            super::uninstall_linux_ca(&tls, &fingerprint, &roots, &mut |command| {
                assert_eq!(command.get_program(), "trust");
                Err(io::Error::new(io::ErrorKind::NotFound, "absent"))
            })
            .is_err()
        );
        assert!(anchor.exists(), "mismatched anchor is never removed");
        assert!(
            super::uninstall_linux_ca(&tls, &fingerprint, &roots, &mut |_| Ok(false)).is_err(),
            "genuine p11-kit failure is not ignored"
        );
    }

    #[test]
    fn interrupted_cleanup_refuses_prepare_until_exact_reset_finishes() {
        for retired_remains in [true, false] {
            let root = TempDir::new().expect("isolated root");
            let domain = root.path().join("storage");
            let prepare = || {
                IndependentSetupHost::prepare(
                    "storage",
                    &domain,
                    "127.0.0.1:31994".parse().expect("address"),
                    "https://localhost:5173",
                    "/storage",
                    false,
                )
            };
            drop(prepare().expect("prepare").expect("host"));
            let mut reset = IndependentReset::open("storage", &domain, true).expect("reset");
            reset.secrets_removed().expect("no secrets");
            reset.journal.phase = super::Phase::Cleanup;
            reset.save().expect("durable cleanup");
            fs::rename(&reset.directory, &reset.retired).expect("crash after rename");
            if !retired_remains {
                fs::remove_dir_all(&reset.retired).expect("crash after retired removal");
            }
            drop(reset);
            assert!(
                prepare().is_err(),
                "ordinary startup cannot replace reset evidence"
            );
            assert!(
                !domain.with_extension("setup-host").exists(),
                "no new admission or TLS"
            );
            IndependentReset::open("storage", &domain, true)
                .expect("exact retry")
                .finish_with_trust(|_| panic!("trust already removed"))
                .expect("finish cleanup");
            assert!(!domain.with_extension("setup-reset").exists());
            assert!(!domain.with_extension("setup-reset.json").exists());
            drop(
                prepare()
                    .expect("fresh allowed after reset")
                    .expect("fresh host"),
            );
        }
    }

    #[test]
    fn reset_preserves_lost_worker_fences_and_never_regenerates_admission() {
        let root = TempDir::new().expect("isolated root");
        let domain = root.path().join("storage");
        let host = IndependentSetupHost::prepare(
            "storage",
            &domain,
            "127.0.0.1:31995".parse().expect("address"),
            "https://localhost:5173",
            "/storage",
            false,
        )
        .expect("prepare")
        .expect("host");
        drop(host);
        let directory = domain.with_extension("setup-host");
        assert!(IndependentReset::open("idp", &domain, true).is_err());
        let fence = directory.join("initialized.json");
        fs::write(&fence, b"true").expect("lost-worker fence fixture");
        fs::set_permissions(&fence, fs::Permissions::from_mode(0o600)).expect("private fence");
        assert!(IndependentReset::open("storage", &domain, true).is_err());
        assert!(fence.exists());
        assert!(!directory.join("worker.json").exists());
        fs::remove_file(fence).expect("remove only injected fixture fence");
        fs::remove_dir_all(directory.join("setup-admission")).expect("inject admission loss");
        assert!(IndependentReset::open("storage", &domain, true).is_err());
        assert!(
            !directory.join("setup-admission").exists(),
            "no implicit admission regeneration"
        );
        assert!(directory.join("tls/ca.pem").exists());
    }

    #[test]
    fn reset_requires_explicit_trust_removal_and_excludes_a_running_setup_host() {
        let root = TempDir::new().expect("isolated root");
        let domain = root.path().join("storage");
        let host = IndependentSetupHost::prepare(
            "storage",
            &domain,
            "127.0.0.1:31991".parse().expect("address"),
            "https://localhost:5173",
            "/storage",
            false,
        )
        .expect("prepare")
        .expect("host");
        assert!(IndependentReset::open("storage", &domain, false).is_err());
        assert!(IndependentReset::open("storage", &domain, true).is_err());
        drop(host);
        let mut reset = IndependentReset::open("storage", &domain, true).expect("stopped reset");
        reset.secrets_removed().expect("no worker secrets");
        assert!(
            reset
                .finish_with_trust(|_| Err(io::Error::other("consent denied")))
                .is_err()
        );
        assert!(
            domain
                .with_extension("setup-host")
                .join("tls/ca.pem")
                .exists()
        );
        IndependentReset::open("storage", &domain, true)
            .expect("retry")
            .finish_with_trust(|_| Ok(()))
            .expect("explicit trust removal retry");
        assert!(!domain.with_extension("setup-host").exists());
        assert!(!domain.with_extension("setup-reset.json").exists());
        fs::create_dir(&domain).expect("unrelated domain");
        assert!(IndependentReset::open("storage", &domain, true).is_err());
        assert!(domain.exists());
    }
}
