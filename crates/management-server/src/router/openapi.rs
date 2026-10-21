use axum::{Json, extract::State};
use serde_json::Value;

#[utoipa::path(get, path = "/openapi.json", responses((status = 200, description = "OpenAPI specification")))]
pub(crate) async fn openapi_json(State(openapi): State<Value>) -> Json<Value> {
    Json(openapi)
}
