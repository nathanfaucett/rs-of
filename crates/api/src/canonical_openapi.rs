use axum::{Json, Router, extract::State, routing::get};
use reqwest::Url;
use serde_json::Value;

/// Loads a checked-in contract for a validated owner. A public origin gains its owner mount;
/// a complete public base retains its path, independently of the local mount.
/// Export remains derived from Rust route registrations, not these served artifacts.
pub fn canonical_openapi(owner: &str, api_base_uri: &str, prefix: &str) -> Value {
    let source = match owner {
        "idp" => include_str!("../../../openapi/idp.json"),
        "management" => include_str!("../../../openapi/management.json"),
        "storage" => include_str!("../../../openapi/storage.json"),
        _ => panic!("canonical OpenAPI requires a validated IdP, Management, or Storage owner"),
    };
    let mut document: Value = serde_json::from_str(source).expect("valid canonical owner OpenAPI");
    let base = if prefix != "/"
        && !prefix.is_empty()
        && Url::parse(api_base_uri).is_ok_and(|url| url.path() == "/")
    {
        format!("{}{prefix}", api_base_uri.trim_end_matches('/'))
    } else {
        api_base_uri.to_owned()
    };
    document["servers"] = serde_json::json!([{ "url": base }]);
    document
}

pub fn canonical_openapi_router<S: Clone + Send + Sync + 'static>(
    owner: &str,
    api_base_uri: &str,
) -> Router<S> {
    Router::new()
        .route(
            "/openapi.json",
            get(|State(document): State<Value>| async { Json(document) }),
        )
        .with_state(canonical_openapi(owner, api_base_uri, ""))
}

#[cfg(test)]
mod tests {
    #[test]
    fn canonical_openapi_normalizes_only_origin_server_urls() {
        for (base, prefix, expected) in [
            ("https://node.example", "/idp", "https://node.example/idp"),
            (
                "https://node.example/idp",
                "/idp",
                "https://node.example/idp",
            ),
            (
                "https://node.example/public/identity",
                "/idp",
                "https://node.example/public/identity",
            ),
            ("https://node.example", "/", "https://node.example"),
        ] {
            let document = super::canonical_openapi("idp", base, prefix);
            assert_eq!(document["servers"][0]["url"], expected);
        }
    }
}
