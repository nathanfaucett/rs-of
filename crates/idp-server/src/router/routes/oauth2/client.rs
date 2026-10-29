use axum::{
    Json,
    extract::{Path, State},
};
use idp_model::contract::{ClientConsentMetadata, ErrorResponse};

use crate::router::{RouterState, middleware::StandardAuthorization};

#[utoipa::path(get, path = "/oauth2/client/{client_id}",
    params(("client_id" = String, Path)),
    responses((status = 200, description = "Public client consent metadata", body = ClientConsentMetadata)),
    security(("authorization" = [])))]
pub(crate) async fn get_client(
    State(state): State<RouterState>,
    Path(client_id): Path<String>,
    _actor: StandardAuthorization,
) -> Result<Json<ClientConsentMetadata>, ErrorResponse> {
    state
        .oauth2_service
        .get_client_consent_metadata(&client_id)
        .await
        .map(Json)
}
