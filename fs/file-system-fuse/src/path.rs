use std::ffi::OsStr;

use fuse3::{Errno, Result};

pub fn relative_path(path: &OsStr) -> Result<String> {
    let path = path.to_str().ok_or_else(|| Errno::from(libc::EILSEQ))?;
    let relative = path
        .strip_prefix('/')
        .ok_or_else(|| Errno::from(libc::EINVAL))?;
    if !relative.is_empty()
        && relative
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".." | ".data"))
    {
        return Err(Errno::from(libc::EINVAL));
    }
    Ok(relative.to_owned())
}

pub fn child_path(parent: &OsStr, name: &OsStr) -> Result<String> {
    let parent = relative_path(parent)?;
    let name = name.to_str().ok_or_else(|| Errno::from(libc::EILSEQ))?;
    if name.is_empty() || name.contains('/') || matches!(name, "." | ".." | ".data") {
        return Err(Errno::from(libc::EINVAL));
    }
    if parent.is_empty() {
        Ok(name.to_owned())
    } else {
        Ok(format!("{parent}/{name}"))
    }
}
