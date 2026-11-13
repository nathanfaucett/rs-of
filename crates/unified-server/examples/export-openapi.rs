use std::{collections::BTreeMap, env, fs, path::PathBuf};

use serde_json::Value;
use utoipa::openapi::{OpenApi, Server};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = PathBuf::from(env::args_os().nth(1).expect("output directory argument"));
    let documents = canonical_documents();
    fs::create_dir_all(&out)?;
    for (name, mut document) in documents {
        document.servers = Some(vec![Server::new(format!("https://localhost/{name}"))]);
        write(&out, name, document)?;
    }
    Ok(())
}

pub fn canonical_documents() -> BTreeMap<&'static str, OpenApi> {
    let mut documents = BTreeMap::from([
        ("idp", idp_server::openapi_document("", "/idp")),
        (
            "management",
            management_server::openapi_document("", "/management"),
        ),
        ("device", device_server::openapi_document("", "/device")),
        ("storage", storage_server::openapi_document("", "/storage")),
    ]);
    for owner in ["idp", "device", "storage"] {
        let document = documents.get_mut(owner).expect("canonical owner");
        merge(
            document,
            server_common::owner_setup_openapi(),
            "owner_setup",
            None,
        );
        merge(
            document,
            device_service::join_history::history_openapi(),
            "independent_join",
            None,
        );
    }
    merge(
        documents.get_mut("idp").expect("IdP"),
        idp_server::independent_join_openapi(),
        "independent_join",
        None,
    );
    merge(
        documents.get_mut("device").expect("Device"),
        device_server::independent_join_openapi(),
        "independent_join",
        None,
    );
    documents
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

fn merge(target: &mut OpenApi, mut source: OpenApi, tag: &str, prefix: Option<&str>) {
    let paths = std::mem::take(&mut source.paths.paths);
    for (path, mut item) in paths {
        for operation in [
            &mut item.get,
            &mut item.put,
            &mut item.post,
            &mut item.delete,
            &mut item.options,
            &mut item.head,
            &mut item.patch,
            &mut item.trace,
        ]
        .into_iter()
        .flatten()
        {
            let id = operation.operation_id.as_deref().expect("operation ID");
            operation.operation_id = Some(format!("{tag}_{id}"));
            operation.tags = Some(vec![tag.to_owned()]);
        }
        let path = prefix.map_or(path.as_str(), |prefix| {
            path.strip_prefix(prefix).expect("owner prefix")
        });
        assert!(
            !target.paths.paths.contains_key(path),
            "duplicate owner route {path}"
        );
        source.paths.paths.insert(path.to_owned(), item);
    }
    target.merge(source);
}

#[cfg(test)]
mod tests {
    #[test]
    fn canonical_exports_keep_all_published_paths_and_oauth_request_schemas() {
        let documents = super::canonical_documents();
        for owner in ["idp", "management", "device", "storage"] {
            let published = server_common::canonical_openapi(owner, "https://localhost", "");
            for path in published["paths"]
                .as_object()
                .expect("published owner paths")
                .keys()
            {
                assert!(
                    documents[owner].paths.paths.contains_key(path),
                    "{owner}: lost published path {path}"
                );
            }
        }
        let idp = serde_json::to_value(&documents["idp"]).expect("IdP contract");
        assert_eq!(
            idp["paths"]["/users/{user_id}"]["put"]["requestBody"]["content"]["application/json"]["schema"]
                ["$ref"],
            "#/components/schemas/UpdateUserInfoRequest"
        );
    }

    #[test]
    fn canonical_exports_preserve_independent_owner_operations() {
        let documents = super::canonical_documents();
        for owner in ["idp", "device", "storage"] {
            let paths = &documents[owner].paths.paths;
            for path in [
                "/setup/owner/status",
                "/setup/owner/create",
                "/setup/owner/join",
                "/independent/node-joins/history/request",
                "/independent/node-joins/history/stream",
            ] {
                assert!(paths.contains_key(path), "{owner}: missing {path}");
            }
        }
        assert!(
            !documents["management"]
                .paths
                .paths
                .contains_key("/setup/owner/status")
        );
    }
}
