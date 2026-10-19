use std::{io, path::Path};

use serde_json::Value;

fn unsupported() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "the shared native host requires Unix protected-state support; remote HTTPS APIs remain independent",
    )
}

pub(crate) fn pending(_: &Path) -> io::Result<bool> {
    Ok(false)
}
pub(crate) fn read_optional(_: &Path) -> io::Result<Option<Vec<u8>>> {
    Err(unsupported())
}
pub(crate) fn publish(_: &Path, _: &str, _: &Value) -> io::Result<()> {
    Err(unsupported())
}
pub(crate) async fn run(_: &Path) -> io::Result<()> {
    Err(unsupported())
}
pub(crate) fn recover(_: &Path) -> io::Result<()> {
    Err(unsupported())
}
pub(crate) fn revocations(_: &Path) -> io::Result<Vec<Value>> {
    Err(unsupported())
}
