use std::{collections::BTreeMap, env, fs, path::PathBuf};

use serde_json::Value;
use utoipa::openapi::{OpenApi, Server};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = PathBuf::from(env::args_os().nth(1).expect("output directory argument"));
    let documents = BTreeMap::from([
        ("idp", idp_server::openapi_document("", "/idp")),
        (
            "management",
            management_server::openapi_document("", "/management"),
        ),
        ("device", device_server::openapi_document("", "/device")),
        ("storage", storage_server::openapi_document("", "/storage")),
    ]);

    fs::create_dir_all(&out)?;
    for (name, mut document) in documents {
        let base = match name {
            "idp" => "/idp",
            "management" => "/management",
            "device" => "/device",
            "storage" => "/storage",
            _ => "/",
        };
        document.servers = Some(vec![Server::new(format!("https://localhost{base}"))]);
        write(&out, name, document)?;
    }
    Ok(())
}
fn write(
    out: &std::path::Path,
    name: &str,
    doc: OpenApi,
) -> Result<(), Box<dyn std::error::Error>> {
    let value: Value = serde_json::to_value(doc)?;
    let mut bytes = serde_json::to_vec_pretty(&value)?;
    bytes.push(b'\n');
    fs::write(out.join(format!("{name}.json")), bytes)?;
    Ok(())
}
