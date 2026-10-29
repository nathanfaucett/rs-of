use std::{
    fs::{self, OpenOptions},
    future::Future,
    io::{self, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Component, Path},
    pin::Pin,
};

use serde_json::{Value, json};
use unified_server::BootstrapConfig;

const JOURNAL: &str = "reset-state.json";
const EVIDENCE: &str = "setup-reference-backup.json";

pub(crate) fn pending(data: &Path) -> io::Result<bool> {
    Ok(read_optional(&data.join(JOURNAL))?.is_some())
}

pub(crate) fn read_optional(path: &Path) -> io::Result<Option<Vec<u8>>> {
    for ancestor in path.ancestors().skip(1) {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => (),
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
            _ => {
                return Err(io::Error::other(
                    "native state ancestry must not contain symlinks",
                ));
            }
        }
    }
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !metadata.file_type().is_file()
        || metadata.nlink() != 1
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o077 != 0
    {
        return Err(io::Error::other(
            "native state must be an owner-only regular file",
        ));
    }
    fs::read(path).map(Some)
}

pub(crate) fn publish(data: &Path, name: &str, value: &Value) -> io::Result<()> {
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(io::Error::other("invalid native state filename"));
    }
    read_optional(&data.join(name))?;
    let temporary = data.join(format!(".{name}-{}.tmp", idp_model::model::Id::now_v7()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)?;
    file.write_all(&serde_json::to_vec(value)?)?;
    file.sync_all()?;
    fs::rename(temporary, data.join(name))?;
    fs::File::open(data)?.sync_all()?;
    if read_optional(&data.join(name))?.as_deref() != Some(serde_json::to_vec(value)?.as_slice()) {
        return Err(io::Error::other("native state durable readback mismatch"));
    }
    Ok(())
}

fn read_record(data: &Path) -> io::Result<Option<Value>> {
    let Some(bytes) = read_optional(&data.join(JOURNAL))? else {
        return Ok(None);
    };
    let record: Value = serde_json::from_slice(&bytes)?;
    if record.as_object().map(|fields| fields.len()) != Some(3)
        || record["version"] != 1
        || !matches!(
            record["phase"].as_str(),
            Some("secrets-pending" | "data-pending")
        )
        || !record["references"].is_object()
    {
        return Err(io::Error::other(
            "invalid native reset journal; exact recovery required",
        ));
    }
    Ok(Some(record))
}

pub(crate) async fn run(data: &Path) -> io::Result<()> {
    run_with(data, |host, references| {
        Box::pin(async move { host.delete_local_setup_secrets(&references).await })
    })
    .await
}

async fn run_with(
    data: &Path,
    delete: impl for<'a> FnOnce(
        &'a BootstrapConfig,
        Value,
    ) -> Pin<Box<dyn Future<Output = io::Result<()>> + Send + 'a>>,
) -> io::Result<()> {
    let root = data.join("node");
    let mut record = match read_record(data)? {
        Some(record) => record,
        None => {
            let host = BootstrapConfig::open(&root)?;
            let references = host.local_reset_references()?;
            let backup = json!({"version":1, "references":references});
            match read_optional(&data.join(EVIDENCE))? {
                Some(bytes) if serde_json::from_slice::<Value>(&bytes)? != backup => {
                    return Err(io::Error::other(
                        "native reset backup conflicts with current host",
                    ));
                }
                Some(_) => (),
                None => publish(data, EVIDENCE, &backup)?,
            }
            let record = json!({"version":1, "phase":"secrets-pending", "references":references});
            publish(data, JOURNAL, &record)?;
            record
        }
    };
    if record["references"]["root"] != json!(root) {
        return Err(io::Error::other(
            "native reset root conflicts with retained host",
        ));
    }
    retain_revocation(data, &record["references"])?;
    let mut host_guard = None;
    if record["phase"] == "secrets-pending" {
        host_guard = Some(BootstrapConfig::open(&root)?);
        let host = host_guard.as_ref().expect("stopped reset host guard");
        if host.local_reset_references()? != record["references"] {
            return Err(io::Error::other(
                "native reset identity changed; recover exact references",
            ));
        }
        delete(host, record["references"].clone()).await?;
        record["phase"] = json!("data-pending");
        publish(data, JOURNAL, &record)?;
    }
    let _data_guard =
        if host_guard.is_none() && read_optional(&root.join("operation.lock"))?.is_some() {
            Some(lock_stopped_host(&root)?)
        } else {
            None
        };
    match fs::symlink_metadata(&root) {
        Ok(metadata) if metadata.file_type().is_dir() => fs::remove_dir_all(&root)?,
        Ok(_) => return Err(io::Error::other("native host root must be a directory")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => (),
        Err(error) => return Err(error),
    }
    drop(host_guard);
    if read_optional(&data.join(EVIDENCE))?.is_some() {
        fs::remove_file(data.join(EVIDENCE))?;
    }
    fs::File::open(data)?.sync_all()?;
    fs::remove_file(data.join(JOURNAL))?;
    fs::File::open(data)?.sync_all()
}

pub(crate) fn recover(data: &Path) -> io::Result<()> {
    let backup: Value = serde_json::from_slice(
        &read_optional(&data.join(EVIDENCE))?
            .ok_or_else(|| io::Error::other("no exact native reset backup retained"))?,
    )?;
    if backup["version"] != 1 || backup["references"]["root"] != json!(data.join("node")) {
        return Err(io::Error::other("invalid native reset backup"));
    }
    read_optional(&data.join("node/host.redb"))?.ok_or_else(|| io::Error::other(
        "missing host SQL database; reference recovery cannot recreate workflow or admission; restore the exact database from an operator backup",
    ))?;
    let host = BootstrapConfig::open(&data.join("node"))?;
    if host.local_reset_references()? != backup["references"] {
        return Err(io::Error::other(
            "recovered references do not match exact owner state",
        ));
    }
    Ok(())
}

fn lock_stopped_host(root: &Path) -> io::Result<fs::File> {
    let path = root.join("operation.lock");
    read_optional(&path)?.ok_or_else(|| {
        io::Error::other("missing stopped host lock; explicit independent recovery required")
    })?;
    let operation = OpenOptions::new().read(true).write(true).open(path)?;
    operation.try_lock().map_err(|error| match error {
        fs::TryLockError::WouldBlock => io::Error::new(
            io::ErrorKind::WouldBlock,
            "native host is still owned by a running worker",
        ),
        fs::TryLockError::Error(error) => error,
    })?;
    Ok(operation)
}

fn validate_revocation(target: &Value) -> io::Result<idp_model::model::Id> {
    let member: idp_model::model::Id = serde_json::from_value(target["member_id"].clone())?;
    let issuer = reqwest::Url::parse(
        target["issuer"]
            .as_str()
            .ok_or_else(|| io::Error::other("missing reset issuer"))?,
    )
    .map_err(io::Error::other)?;
    if target.as_object().map(|fields| fields.len()) != Some(4)
        || target["version"] != 1
        || member.is_nil()
        || !matches!(target["status"].as_str(), Some("pending" | "acknowledged"))
        || issuer.scheme() != "https"
        || issuer.host_str().is_none()
        || !issuer.username().is_empty()
        || issuer.password().is_some()
        || issuer.query().is_some()
        || issuer.fragment().is_some()
    {
        return Err(io::Error::other("invalid reset revocation target"));
    }
    Ok(member)
}

fn retain_revocation(data: &Path, references: &Value) -> io::Result<()> {
    if references["keys"].is_null() {
        return Ok(());
    }
    let member = references["keys"]["member_id"]
        .as_str()
        .ok_or_else(|| io::Error::other("missing reset Device"))?;
    let name = format!("reset-revocation-{member}.json");
    if let Some(bytes) = read_optional(&data.join(&name))? {
        let target: Value = serde_json::from_slice(&bytes)?;
        validate_revocation(&target)?;
        if target["issuer"] != references["issuer"] || target["member_id"] != member {
            return Err(io::Error::other(
                "retained revocation target conflicts with reset identity",
            ));
        }
    } else {
        let target = json!({"version":1, "issuer":references["issuer"], "member_id":member, "status":"pending"});
        validate_revocation(&target)?;
        publish(data, &name, &target)?;
    }
    Ok(())
}

pub(crate) fn revocations(data: &Path) -> io::Result<Vec<Value>> {
    let mut targets = Vec::new();
    for entry in fs::read_dir(data)? {
        let path = entry?.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("reset-revocation-") && name.ends_with(".json"))
        {
            let target: Value = serde_json::from_slice(
                &read_optional(&path)?.ok_or_else(|| io::Error::other("lost reset target"))?,
            )?;
            let member = validate_revocation(&target)?;
            if path.file_name().and_then(|name| name.to_str())
                != Some(format!("reset-revocation-{member}.json").as_str())
            {
                return Err(io::Error::other(
                    "reset revocation filename conflicts with member",
                ));
            }
            targets.push(target);
        }
    }
    Ok(targets)
}

#[cfg(test)]
#[path = "setup_reset_tests.rs"]
mod tests;
