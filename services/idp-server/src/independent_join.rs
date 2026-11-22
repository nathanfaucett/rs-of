#[cfg(feature = "independent")]
use std::{io, path::Path, sync::Arc};

use axum::Router;
use device_service::independent_join::IndependentJoinApproval;
use device_service::join_history::CheckJoinHistory;
use device_service::join_history::JoinHistoryContext;
use device_service::join_history::JoinHistorySource;
use ofdb_sql::Database;
use tokio_util::sync::CancellationToken;

use crate::{config::app_config::AppConfig, router::state::RouterState, runtime::IdpRuntime};

pub async fn build_independent_idp(
    config: &AppConfig,
    engine: ofdb_sql::Client,
    owner: Option<Arc<Database>>,
    cancellation: CancellationToken,
    endpoint: String,
) -> io::Result<IdpRuntime> {
    if config.identity_database.is_some() && owner.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "injected identity owner and remote SQL are mutually exclusive",
        ));
    }
    let permissions = management_service::LocalPermissions::new(engine.clone());
    let state = crate::runtime::build_state(config, engine, None, None, Some(permissions)).await?;
    let device = config
        .control_plane_uri
        .clone()
        .ok_or_else(|| io::Error::other("independent Device binding missing"))?;
    let source = Arc::new(Source {
        state: state.clone(),
        endpoint,
        device,
        client: config
            .setup_http_client
            .clone()
            .map(Ok)
            .unwrap_or_else(setup::local_tls::LocalTls::verification_client)?,
    });
    let checked = source.clone();
    let check: CheckJoinHistory = Arc::new(move |approval| {
        let checked = checked.clone();
        Box::pin(async move { checked.verify(&approval).await })
    });
    let mut routes = Router::new();
    if let Some(owner) = owner {
        let history = JoinHistorySource::new(
            owner,
            JoinHistoryContext::Idp,
            Path::new(&config.data_dir).join("join-source-history"),
            check,
            cancellation,
        );
        routes = routes.merge(history.router());
    }
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
    device: String,
    client: reqwest::Client,
}
impl Source {
    fn binding(&self, approval: &IndependentJoinApproval) -> io::Result<()> {
        approval.verify()?;
        if approval.request.source.endpoint != self.endpoint
            || approval.request.source.device != self.device
            || approval.request.source.idp != self.state.api_base_uri
            || approval.request.operation.issuer
                != self
                    .state
                    .oauth2_service
                    .metadata(&self.state.api_base_uri)
                    .issuer
        {
            return Err(io::Error::other("source Join binding mismatch"));
        }
        Ok(())
    }
    async fn device(&self, approval: &IndependentJoinApproval) -> io::Result<()> {
        self.binding(approval)?;
        let configuration = of_device_client::apis::configuration::Configuration {
            base_path: self.device.clone(),
            client: self.client.clone(),
            ..Default::default()
        };
        of_device_client::apis::independent_join_api::independent_join_verify(
            &configuration,
            of_device_client::apis::independent_join_api::IndependentJoinVerifyParams {
                verification_request: serde_json::from_value(
                    serde_json::json!({"approval":approval,"context":"idp"}),
                )?,
            },
        )
        .await
        .map_err(|_| io::Error::other("current Management enrollment unavailable or denied"))
    }
    async fn verify(&self, approval: &IndependentJoinApproval) -> io::Result<()> {
        self.device(approval).await?;
        self.state
            .oauth2_service
            .find_user_info(approval.owner_subject)
            .await
            .map_err(|_| io::Error::other("approved owner is not active"))?;
        Ok(())
    }
}
