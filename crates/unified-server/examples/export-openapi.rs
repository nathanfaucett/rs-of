use std::{collections::BTreeMap, env, fs, path::PathBuf};

use serde_json::Value;
use utoipa::openapi::{OpenApi, Server};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = PathBuf::from(env::args_os().nth(1).expect("output directory argument"));
    let mut documents = BTreeMap::from([
        ("idp", idp_server::openapi_document("", "/idp")),
        (
            "management",
            management_server::openapi_document("", "/management"),
        ),
        ("storage", storage_server::openapi_document("", "/storage")),
    ]);
    for owner in ["idp", "management", "storage"] {
        let document = documents.get_mut(owner).expect("canonical owner");
        merge(document, api::owner_setup_openapi(), "owner_setup", None);
        merge(
            document,
            api::independent_setup_openapi(),
            "independent_setup",
            None,
        );
        merge(
            document,
            management_service::join_history::history_openapi(),
            "independent_join",
            None,
        );
    }
    merge(
        documents.get_mut("idp").expect("IdP"),
        unified_server::setup_openapi(),
        "setup",
        Some("/idp"),
    );
    merge(
        documents.get_mut("idp").expect("IdP"),
        idp_server::independent_join_openapi(),
        "independent_join",
        None,
    );
    merge(
        documents.get_mut("management").expect("Management"),
        unified_server::node_join_openapi(),
        "node_join",
        Some("/management"),
    );
    merge(
        documents.get_mut("management").expect("Management"),
        management_server::independent_join_openapi(),
        "independent_join",
        None,
    );
    fs::create_dir_all(&out)?;
    for (name, mut document) in documents {
        let base = match name {
            "idp" => "/idp",
            "management" => "/management",
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
