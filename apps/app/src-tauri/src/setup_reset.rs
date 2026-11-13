use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Component, Path},
};

use serde_json::{Value, json};

use crate::desktop_config::DesktopConfig;

const JOURNAL: &str = "reset-state.json";

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
    let temporary = data.join(format!(
        ".{name}-{}.tmp",
        of_model::idp::model::Id::now_v7()
    ));
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

pub(crate) async fn run(data: &Path) -> io::Result<()> {
    let root = data.join("node");
    let config = DesktopConfig::open(&root)?;
    let path = data.join(JOURNAL);
    let mut completed = match read_optional(&path)? {
        Some(bytes) => serde_json::from_slice::<Vec<String>>(&bytes)?,
        None => Vec::new(),
    };
    if completed
        .iter()
        .any(|owner| !matches!(owner.as_str(), "idp" | "device" | "storage"))
    {
        return Err(io::Error::other(
            "unsupported reset evidence; preserve the root and restore the previous binary for exact reset/recovery",
        ));
    }
    publish(data, JOURNAL, &json!(completed))?;
    if !completed.iter().any(|owner| owner == "idp")
        || read_optional(&Path::new(&config.idp.data_dir).with_extension("setup-reset.json"))?
            .is_some()
    {
        let reset = server_common::IndependentReset::open(
            "idp",
            std::path::Path::new(&config.idp.data_dir),
            true,
        )?;
        if let (Some(worker), Some(intent)) = (reset.worker(), reset.intent()) {
            if let Some(member) = worker["member"].as_str() {
                let target = json!({"version":1, "issuer":intent.issuer, "member_id":member, "status":"pending"});
                validate_revocation(&target)?;
                let name = format!("reset-revocation-{member}.json");
                match read_optional(&data.join(&name))? {
                    Some(bytes) if serde_json::from_slice::<Value>(&bytes)? != target => {
                        return Err(io::Error::other(
                            "retained revocation target conflicts with owner identity",
                        ));
                    }
                    Some(_) => (),
                    None => publish(data, &name, &target)?,
                }
            }
        }
        drop(reset);
        idp_server::reset_independent(&config.idp, true, || {
            if !completed.iter().any(|owner| owner == "idp") {
                completed.push("idp".into());
            }
            publish(data, JOURNAL, &json!(completed))
        })
        .await?;
    }
    if !completed.iter().any(|owner| owner == "device")
        || read_optional(&Path::new(&config.device.data_dir).with_extension("setup-reset.json"))?
            .is_some()
    {
        device_server::reset_independent(&config.device, true, || {
            if !completed.iter().any(|owner| owner == "device") {
                completed.push("device".into());
            }
            publish(data, JOURNAL, &json!(completed))
        })
        .await?;
    }
    if !completed.iter().any(|owner| owner == "storage")
        || read_optional(&Path::new(&config.storage.data_dir).with_extension("setup-reset.json"))?
            .is_some()
    {
        storage_server::reset_independent(&config.storage, true, || {
            if !completed.iter().any(|owner| owner == "storage") {
                completed.push("storage".into());
            }
            publish(data, JOURNAL, &json!(completed))
        })
        .await?;
    }
    fs::File::open(data)?.sync_all()?;
    fs::remove_file(path)?;
    fs::File::open(data)?.sync_all()
}

pub(crate) fn recover(data: &Path) -> io::Result<()> {
    if read_optional(&data.join("setup-reference-backup.json"))?.is_some() {
        return Err(io::Error::other(
            "unsupported previous Unified Host reference backup; preserve it and use the previous binary for exact recovery/reset",
        ));
    }
    let root = data.join("node");
    for owner in ["idp", "device", "storage"] {
        if !root.join(format!("{owner}.setup-host")).is_dir() {
            return Err(io::Error::other(
                "owner-local setup evidence is missing; restore the exact owner root from backup; reference recovery cannot synthesize setup history",
            ));
        }
    }
    drop(crate::native_host::prepare(&root)?);
    Ok(())
}

fn validate_revocation(target: &Value) -> io::Result<of_model::idp::model::Id> {
    let member: of_model::idp::model::Id = serde_json::from_value(target["member_id"].clone())?;
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
