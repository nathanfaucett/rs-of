use axum::Json;
use of_model::contract::VersionResponse;

#[utoipa::path(get, path = "/version", responses((status = 200, description = "Version", body = VersionResponse)))]
pub(crate) async fn version() -> Json<VersionResponse> {
    #[cfg(feature = "sync")]
    let features = vec!["sync".to_string()];
    #[cfg(not(feature = "sync"))]
    let features = Vec::new();

    Json(VersionResponse {
        name: env!("CARGO_PKG_NAME").to_string(),
        features,
        ..VersionResponse::default()
    })
}
