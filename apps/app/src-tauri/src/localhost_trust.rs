use std::{
    io,
    path::Path,
    process::{Command, Stdio},
};

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn linux_ca_trust_identifier(ca_pem: &[u8]) -> io::Result<String> {
    use std::fmt::Write;

    use rustls::pki_types::{CertificateDer, pem::PemObject};

    let certificate = CertificateDer::from_pem_slice(ca_pem).map_err(io::Error::other)?;
    let digest = rustls::crypto::ring::cipher_suite::TLS13_AES_128_GCM_SHA256
        .tls13()
        .expect("TLS13_AES_128_GCM_SHA256 is a TLS 1.3 suite")
        .common
        .hash_provider
        .hash(certificate.as_ref());
    let mut fingerprint = String::with_capacity(64);
    for byte in digest.as_ref() {
        write!(fingerprint, "{byte:02x}").expect("writing to a String cannot fail");
    }
    Ok(format!("aicacia-localhost-ca-{fingerprint}.crt"))
}

fn trust_command(program: &str) -> Command {
    let mut command = Command::new(program);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

pub fn install_ca_to_user_trust_store(ca_pem_path: &Path) -> io::Result<()> {
    let result = install_ca_with_commands(ca_pem_path, &mut run_trust_command);
    match &result {
        Ok(()) => log::info!("installed localhost CA into the operating system trust store"),
        Err(error) => log::error!("localhost OS CA trust installation failed: {error}"),
    }
    result
}

fn run_trust_command(command: &mut Command) -> io::Result<bool> {
    #[cfg(test)]
    {
        let _ = command;
        Err(io::Error::other(
            "OS trust commands are disabled in tests; inject a command runner",
        ))
    }
    #[cfg(not(test))]
    {
        command.status().map(|status| status.success())
    }
}

fn install_ca_with_commands(
    ca_pem_path: &Path,
    run: &mut impl FnMut(&mut Command) -> io::Result<bool>,
) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").map_err(io::Error::other)?;
        let keychain = format!("{home}/Library/Keychains/login.keychain-db");
        let mut command = trust_command("security");
        command
            .args(["add-trusted-cert", "-r", "trustRoot", "-p", "ssl", "-k"])
            .arg(keychain)
            .arg(ca_pem_path);
        if run(&mut command)? {
            return Ok(());
        }
    }
    #[cfg(target_os = "windows")]
    {
        let mut command = trust_command("certutil");
        command
            .args(["-addstore", "-user", "Root"])
            .arg(ca_pem_path);
        if run(&mut command)? {
            return Ok(());
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let identifier = linux_ca_trust_identifier(&std::fs::read(ca_pem_path)?)?;
        for (directory, refresh) in [
            (
                "/usr/local/share/ca-certificates",
                &["update-ca-certificates"][..],
            ),
            (
                "/etc/pki/ca-trust/source/anchors",
                &["update-ca-trust", "extract"][..],
            ),
        ] {
            let mut install = trust_command("pkexec");
            install
                .args(["install", "-m", "0644"])
                .arg(ca_pem_path)
                .arg(Path::new(directory).join(&identifier));
            if !run(&mut install).unwrap_or(false) {
                continue;
            }
            let mut update = trust_command("pkexec");
            update.args(refresh);
            if run(&mut update).unwrap_or(false) {
                return Ok(());
            }
        }
    }
    Err(io::Error::other(
        "could not install the localhost CA into the operating system trust store",
    ))
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, fs, io, path::Path};

    use super::install_ca_with_commands;

    #[test]
    fn os_trust_failures_never_succeed() {
        let directory = std::env::temp_dir().join(format!("trust-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory).expect("isolated trust fixture");
        let source = directory.join("localhost-ca.pem");
        fs::write(
            &source,
            b"-----BEGIN CERTIFICATE-----\nYWJj\n-----END CERTIFICATE-----\n",
        )
        .expect("fixture certificate");
        for unavailable in [false, true] {
            let mut calls = Vec::new();
            let result = install_ca_with_commands(&source, &mut |command| {
                calls.push((
                    command.get_program().to_owned(),
                    command.get_args().map(OsString::from).collect::<Vec<_>>(),
                ));
                if unavailable {
                    Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        "command unavailable",
                    ))
                } else {
                    Ok(false)
                }
            });
            assert!(result.is_err());
            assert!(!calls.is_empty());
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            assert!(
                calls
                    .iter()
                    .all(|(program, args)| program == "pkexec" && args[0] == "install")
            );
        }
        assert!(install_ca_with_commands(&source, &mut |_| Ok(true)).is_ok());
        fs::remove_dir_all(directory).expect("remove trust fixture");
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    #[test]
    fn linux_requires_matching_os_anchor_copy_and_refresh() {
        let directory = std::env::temp_dir().join(format!("trust-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory).expect("isolated trust fixture");
        let source = directory.join("localhost-ca.pem");
        fs::write(
            &source,
            b"-----BEGIN CERTIFICATE-----\nYWJj\n-----END CERTIFICATE-----\n",
        )
        .expect("fixture certificate");
        let identifier = super::linux_ca_trust_identifier(&fs::read(&source).expect("fixture PEM"))
            .expect("decoded DER fingerprint");
        assert_eq!(
            identifier,
            "aicacia-localhost-ca-ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad.crt"
        );
        assert!(super::linux_ca_trust_identifier(b"invalid PEM").is_err());
        for outcomes in [
            vec![true, true],
            vec![false, true, true],
            vec![true, false, true, false],
            vec![true, false, false],
        ] {
            let mut calls = Vec::new();
            let mut results = outcomes.iter().copied();
            let result = install_ca_with_commands(&source, &mut |command| {
                assert_eq!(command.get_program(), "pkexec");
                calls.push(command.get_args().map(OsString::from).collect::<Vec<_>>());
                Ok(results.next().expect("no unexpected fallback commands"))
            });
            assert_eq!(
                result.is_ok(),
                outcomes == [true, true] || outcomes == [false, true, true]
            );
            assert!(results.next().is_none());
            assert_eq!(
                calls[0],
                [
                    OsString::from("install"),
                    OsString::from("-m"),
                    OsString::from("0644"),
                    source.as_os_str().to_owned(),
                    Path::new("/usr/local/share/ca-certificates")
                        .join(&identifier)
                        .into_os_string()
                ]
            );
            for (index, args) in calls.iter().enumerate() {
                if args[0] != "install" {
                    let anchor = &calls[index - 1][4];
                    if anchor
                        == &Path::new("/usr/local/share/ca-certificates")
                            .join(&identifier)
                            .into_os_string()
                    {
                        assert_eq!(args, &[OsString::from("update-ca-certificates")]);
                    } else {
                        assert_eq!(
                            args,
                            &[OsString::from("update-ca-trust"), OsString::from("extract")]
                        );
                    }
                }
                if args[0] == "install" && args != &calls[0] {
                    assert_eq!(
                        args[4],
                        Path::new("/etc/pki/ca-trust/source/anchors")
                            .join(&identifier)
                            .into_os_string()
                    );
                }
            }
        }
        fs::remove_dir_all(directory).expect("remove trust fixture");
    }
}
