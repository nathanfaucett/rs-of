use std::{
    io,
    path::Path,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
};
use db::Database;
use idp_model::contract::{IdpSignerRecord, ReplicaMembership};
use management_service::{
    independent_join::IndependentJoinApproval,
    join_history::{CheckJoinHistory, JoinHistoryContext, JoinHistorySource},
};
use tokio_util::sync::CancellationToken;

use crate::{AppConfig, IdpRuntime, RouterState};

pub async fn build_independent_idp(
    config: &AppConfig,
    engine: Arc<Database>,
    cancellation: CancellationToken,
) -> io::Result<IdpRuntime> {
    let state = crate::runtime::build_state(config, engine.clone(), None, None, None).await?;
    let endpoint = config
        .oauth2
        .local_signer
        .as_ref()
        .ok_or_else(|| io::Error::other("independent signer binding missing"))?
        .endpoint_id
        .clone();
    let management = config
        .management_permissions
        .as_ref()
        .ok_or_else(|| io::Error::other("independent Management binding missing"))?
        .api_base_uri
        .clone();
    let source = Arc::new(Source {
        state: state.clone(),
        endpoint,
        management,
    });
    let checked = source.clone();
    let check: CheckJoinHistory = Arc::new(move |approval| {
        let checked = checked.clone();
        Box::pin(async move { checked.verify(&approval).await })
    });
    let history = JoinHistorySource::new(
        engine,
        JoinHistoryContext::Idp,
        Path::new(&config.data_dir).join("join-source-history"),
        check,
        cancellation,
    );
    let routes = Router::new()
        .route("/node-joins/enroll", post(enroll))
        .with_state(source)
        .merge(history.router());
    let routes = if config.server.prefix().is_empty() {
        routes
    } else {
        Router::new().nest(config.server.prefix(), routes)
    };
    let mut runtime = crate::runtime::runtime_from_state(config, state);
    runtime.router = runtime.router.merge(routes);
    Ok(runtime)
}
struct Source {
    state: RouterState,
    endpoint: String,
    management: String,
}
impl Source {
    fn binding(&self, approval: &IndependentJoinApproval) -> io::Result<()> {
        approval.verify()?;
        if approval.request.source.endpoint != self.endpoint
            || approval.request.source.management != self.management
            || approval.request.source.idp != self.state.api_base_uri
            || approval.request.operation.issuer != self.state.oauth2_service.metadata().issuer
        {
            return Err(io::Error::other("source Join binding mismatch"));
        }
        Ok(())
    }
    async fn management(&self, approval: &IndependentJoinApproval) -> io::Result<()> {
        self.binding(approval)?;
        let response = api::LocalTls::verification_client()?
            .post(format!("{}/node-joins/verify", self.management))
            .json(&serde_json::json!({"approval":approval,"context":"idp"}))
            .send()
            .await
            .map_err(|_| io::Error::other("current Management enrollment unavailable"))?;
        if response.status() != StatusCode::NO_CONTENT {
            return Err(io::Error::other("current Management enrollment denied"));
        }
        Ok(())
    }
    async fn verify(&self, approval: &IndependentJoinApproval) -> io::Result<()> {
        self.management(approval).await?;
        self.state
            .oauth2_service
            .find_user_info(approval.owner_subject)
            .await
            .map_err(|_| io::Error::other("approved owner is not active"))?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_secs() as i64;
        let signers = self
            .state
            .replica_signers
            .trusted_signers(&approval.request.operation.issuer, now)
            .await
            .map_err(io::Error::other)?;
        if !signers.iter().any(|(member, signer)| {
            member.member_id == approval.request.member_id
                && member.endpoint_id == approval.request.endpoint_id
                && signer.public_jwk == approval.request.public_signer
        }) {
            return Err(io::Error::other(
                "exact joining signer not currently trusted",
            ));
        }
        Ok(())
    }
}
async fn enroll(
    State(source): State<Arc<Source>>,
    headers: HeaderMap,
    Json(approval): Json<IndependentJoinApproval>,
) -> Result<StatusCode, StatusCode> {
    let bearer = headers
        .get("Authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| !value.is_empty())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let actor = crate::authorize_bearer(&source.state, bearer)
        .await
        .map_err(|_| StatusCode::FORBIDDEN)?;
    crate::router::require_identity_permission_for_audience(
        &source.state,
        &actor,
        management_service::MANAGEMENT_APPLICATION_URI,
        idp_model::contract::IdentityAction::ReplicaSignersEnroll,
        idp_model::contract::PermissionTarget::Installation {
            resource: idp_model::contract::IdentityResource::ReplicaSigner { member_id: None },
        },
    )
    .await
    .map_err(|_| StatusCode::FORBIDDEN)?;

    let owner = actor.principal.get_entity_id();
    if owner != approval.owner_subject {
        return Err(StatusCode::FORBIDDEN);
    }
    source
        .management(&approval)
        .await
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
        .as_secs() as i64;
    let membership = ReplicaMembership {
        installation_id: approval.request.operation.issuer.clone(),
        member_id: approval.request.member_id,
        endpoint_id: approval.request.endpoint_id.clone(),
        issuer: approval.request.operation.issuer.clone(),
        approved_at: now,
        revoked_at: None,
    };
    let signer = IdpSignerRecord {
        member_id: membership.member_id,
        key_id: approval
            .request
            .public_signer
            .kid
            .parse()
            .map_err(|_| StatusCode::BAD_REQUEST)?,
        issuer: membership.issuer.clone(),
        public_jwk: approval.request.public_signer.clone(),
        approved_at: Some(now),
        revoked_at: None,
        expires_at: None,
    };
    source
        .state
        .replica_signers
        .enroll(&membership, &signer)
        .await
        .map_err(|_| StatusCode::CONFLICT)?;
    source
        .verify(&approval)
        .await
        .map_err(|_| StatusCode::FORBIDDEN)?;

    Ok(StatusCode::NO_CONTENT)
}
