use std::{io, path::Path, sync::Arc};

use api::{
    IndependentCreateCommand, IndependentCreateControl, IndependentCreateIntent,
    IndependentOwnerPublic, IndependentSetupHost,
};
use axum::Router;
use db::Database;
use idp_model::{contract::IdentityAction, model::Id};
use idp_service::setup_credentials::{SetupRelationship, create_setup_secret, load_setup_secret};
use management_service::{
    DeviceIdentity, DeviceRepo, HostedControlPlane,
    replica::{DbDeviceRepo, DbSelectionPolicyRepo},
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::AppConfig;

const ACTIONS: [IdentityAction; 25] = [
    IdentityAction::InfrastructureClientsRead,
    IdentityAction::InfrastructureClientsCreate,
    IdentityAction::InfrastructureClientsUpdate,
    IdentityAction::InfrastructureClientsDelete,
    IdentityAction::ApplicationsRead,
    IdentityAction::ApplicationsCreate,
    IdentityAction::ApplicationsUpdate,
    IdentityAction::ApplicationsDelete,
    IdentityAction::UsersRead,
    IdentityAction::UsersUpdate,
    IdentityAction::UsersDelete,
    IdentityAction::UsersResetPassword,
    IdentityAction::ConsentsRead,
    IdentityAction::ConsentsRevoke,
    IdentityAction::KeysRead,
    IdentityAction::KeysRotate,
    IdentityAction::KeysRevoke,
    IdentityAction::DevicesRead,
    IdentityAction::DevicePairingRead,
    IdentityAction::DevicePairingUpdate,
    IdentityAction::ReplicaSignersEnroll,
    IdentityAction::ReplicaSignersRotate,
    IdentityAction::ReplicaSignersRevoke,
    IdentityAction::IdpSecuritySync,
    IdentityAction::ManagementSecuritySync,
];

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u8,
    client: Id,
    device: Id,
    role: Id,
    permissions: Vec<Id>,
    user: Option<Id>,
    evaluator: Option<Id>,
    secret_attempted: bool,
    endpoint_attempted: bool,
    endpoint: Option<String>,
    domain_attempted: bool,
    complete: bool,
}
impl Record {
    fn new() -> Self {
        Self {
            version: 1,
            client: Id::now_v7(),
            device: Id::now_v7(),
            role: Id::now_v7(),
            permissions: ACTIONS.iter().map(|_| Id::now_v7()).collect(),
            user: None,
            evaluator: None,
            secret_attempted: false,
            endpoint_attempted: false,
            endpoint: None,
            domain_attempted: false,
            complete: false,
        }
    }
    fn validate(&self) -> io::Result<()> {
        let mut ids = vec![self.client, self.device, self.role];
        ids.extend(&self.permissions);
        if self.version != 1
            || self.permissions.len() != ACTIONS.len()
            || ids.iter().any(Id::is_nil)
            || ids.iter().enumerate().any(|(i, id)| ids[..i].contains(id))
            || self.user.is_some_and(|id| id.is_nil())
            || self.evaluator.is_some_and(|id| id.is_nil())
            || (self.complete
                && (!self.secret_attempted
                    || !self.endpoint_attempted
                    || !self.domain_attempted
                    || self.endpoint.is_none()
                    || self.user.is_none()
                    || self.evaluator.is_none()))
        {
            return Err(io::Error::other("invalid independent Management evidence"));
        }
        Ok(())
    }
}
fn service(control: &IndependentCreateControl) -> String {
    format!("independent-setup:{}", control.directory().display())
}

pub async fn serve_independent_create(
    config: Arc<AppConfig>,
    setup: IndependentSetupHost,
    shutdown: CancellationToken,
) -> io::Result<()> {
    setup.require_owner("management", Path::new(&config.data_dir))?;
    let (host, commands) = setup.into_create(shutdown.clone())?;
    let mut worker = tokio::spawn(run(config, host.control(), commands, shutdown.clone()));
    let serving = host.serve();
    tokio::pin!(serving);
    tokio::select! {
        result = &mut serving => { shutdown.cancel(); result.and(worker.await.map_err(io::Error::other)?) },
        result = &mut worker => { shutdown.cancel(); result.map_err(io::Error::other)?.and(serving.await) },
    }
}
async fn run(
    config: Arc<AppConfig>,
    control: IndependentCreateControl,
    mut commands: mpsc::Receiver<IndependentCreateCommand>,
    shutdown: CancellationToken,
) -> io::Result<()> {
    let mut engine = None;
    let mut identity = None;
    let mut router = None;
    if let Some(record) = control.read_record::<Record>()? {
        record.validate()?;
        if record.complete {
            let intent = control
                .intent()
                .await
                .ok_or_else(|| io::Error::other("completed Management has no exact operation"))?;
            identity = Some(open_identity(&control, &record).await?);
            engine = Some(open_existing(&config).await?);
            verify(engine.as_ref().expect("owner"), &record).await?;
            public(&control, &intent, &record).await?;
            router = Some(
                build(
                    &config,
                    &control,
                    &intent,
                    &record,
                    engine.as_ref().expect("owner").clone(),
                    identity.as_ref().expect("identity").clone(),
                )
                .await?,
            );
            control
                .restore(router.as_ref().expect("router").clone())
                .await?;
        }
    }
    loop {
        let command = tokio::select! { biased; () = shutdown.cancelled() => break, command = commands.recv() => match command { Some(command) => command, None => break } };
        if let Err(error) = provision(
            &config,
            &control,
            &command,
            &mut engine,
            &mut identity,
            &mut router,
        )
        .await
        {
            log::warn!(
                "Independent Create owner=management state=unfinished error_kind={:?}",
                error.kind()
            );
            control.unfinished().await;
        }
    }
    if let Some(identity) = identity {
        identity.endpoint().close().await;
    }
    Ok(())
}
async fn open_identity(
    control: &IndependentCreateControl,
    record: &Record,
) -> io::Result<Arc<DeviceIdentity>> {
    let secret: iroh::SecretKey = load_setup_secret(&service(control), "endpoint")?
        .parse()
        .map_err(io::Error::other)?;
    if record.endpoint.as_deref() != Some(secret.public().to_string().as_str()) {
        return Err(io::Error::other(
            "Management endpoint private material changed",
        ));
    }
    let endpoint = iroh::Endpoint::builder(iroh::endpoint::presets::Minimal)
        .secret_key(secret.clone())
        .bind()
        .await
        .map_err(io::Error::other)?;
    Ok(Arc::new(DeviceIdentity::new(endpoint, secret)?))
}
async fn open_existing(config: &AppConfig) -> io::Result<Arc<Database>> {
    if !Path::new(&config.data_dir)
        .join("management.redb")
        .is_file()
    {
        return Err(io::Error::other("attempted Management store missing"));
    }
    crate::open_database(config).await
}
async fn verify(engine: &Arc<Database>, record: &Record) -> io::Result<()> {
    let endpoint = record
        .endpoint
        .as_deref()
        .ok_or_else(|| io::Error::other("endpoint missing"))?;
    let user = record
        .user
        .ok_or_else(|| io::Error::other("retained owner missing"))?;
    let device = DbDeviceRepo::new(engine.clone())
        .find_approved_by_public_key(endpoint)
        .await
        .map_err(io::Error::other)?
        .ok_or_else(|| io::Error::other("current approved Management endpoint missing"))?;
    if device.id != record.device || device.owner_subject != user.to_string() {
        return Err(io::Error::other("Management endpoint binding changed"));
    }
    if DbSelectionPolicyRepo::new(engine.clone())
        .get(record.device, &user.to_string(), Id::nil())
        .await
        .map_err(io::Error::other)?
        .is_none()
    {
        return Err(io::Error::other("current local endpoint policy missing"));
    }
    Ok(())
}
async fn public(
    control: &IndependentCreateControl,
    intent: &IndependentCreateIntent,
    record: &Record,
) -> io::Result<()> {
    let client = SetupRelationship::ManagementIdp.registration(
        record.client,
        &intent.idp,
        &intent.management,
        &intent.storage,
    );
    let scope = service(control);
    let id = record.client.to_string();
    control
        .set_public(
            IndependentOwnerPublic {
                operation: intent.clone(),
                owner: "management".into(),
                clients: vec![serde_json::to_value(client)?],
                facts: serde_json::json!({"endpoint":record.endpoint, "device":record.device}),
                registered: Vec::new(),
            },
            Arc::new(move |requested| {
                if requested != id {
                    return Err(io::Error::other("unknown consumer Client"));
                }
                load_setup_secret(&scope, requested)
            }),
        )
        .await
}
async fn build(
    config: &AppConfig,
    control: &IndependentCreateControl,
    intent: &IndependentCreateIntent,
    record: &Record,
    engine: Arc<Database>,
    identity: Arc<DeviceIdentity>,
) -> io::Result<Router> {
    let plane = HostedControlPlane::new_with_services(&intent.idp, &intent.storage, &intent.issuer)
        .and_then(|p| {
            p.with_idp_service_client(
                record.client.to_string(),
                load_setup_secret(&service(control), &record.client.to_string())
                    .map_err(|_| "protected Management Client missing")?,
                &intent.idp,
            )
        })
        .and_then(|p| {
            p.with_permission_evaluator(
                &record
                    .evaluator
                    .ok_or("permission evaluator binding missing")?
                    .to_string(),
            )
        })
        .map_err(io::Error::other)?
        .with_device_identity(identity);
    crate::build_router(
        engine,
        &intent.management,
        config.server.prefix(),
        &intent.storage,
        Arc::new(plane),
    )
    .await
    .map_err(io::Error::other)
}
async fn provision(
    config: &AppConfig,
    control: &IndependentCreateControl,
    command: &IndependentCreateCommand,
    engine: &mut Option<Arc<Database>>,
    identity: &mut Option<Arc<DeviceIdentity>>,
    router: &mut Option<Router>,
) -> io::Result<()> {
    let mut record = match control.read_record::<Record>()? {
        Some(record) => record,
        None => {
            let record = Record::new();
            control.write_record(&record)?;
            record
        }
    };
    record.validate()?;
    if record.complete {
        return control
            .finish(
                command,
                router
                    .as_ref()
                    .ok_or_else(|| io::Error::other("completed Management runtime not reopened"))?
                    .clone(),
                &record,
            )
            .await;
    }
    if !record.secret_attempted {
        record.secret_attempted = true;
        control.write_record(&record)?;
        create_setup_secret(&service(control), &record.client.to_string())?;
    }
    load_setup_secret(&service(control), &record.client.to_string())?;
    if !record.endpoint_attempted {
        record.endpoint_attempted = true;
        control.write_record(&record)?;
        create_setup_secret(&service(control), "endpoint")?;
    }
    let secret: iroh::SecretKey = load_setup_secret(&service(control), "endpoint")?
        .parse()
        .map_err(io::Error::other)?;
    let endpoint = secret.public().to_string();
    if record
        .endpoint
        .as_ref()
        .is_some_and(|saved| saved != &endpoint)
    {
        return Err(io::Error::other("protected endpoint changed"));
    }
    record.endpoint = Some(endpoint);
    control.write_record(&record)?;
    if identity.is_none() {
        *identity = Some(open_identity(control, &record).await?);
    }
    public(control, &command.intent, &record).await?;
    control
        .wait_registered(&record.client.to_string(), &command.cancellation)
        .await?;
    let idp = control
        .wait_peer(&command.intent, "idp", &command.cancellation)
        .await?;
    let user: Id = serde_json::from_value(idp.facts["user"].clone())?;
    let evaluator: Id = serde_json::from_value(idp.facts["evaluator"].clone())?;
    if user.is_nil()
        || evaluator.is_nil()
        || idp.facts["endpoint"].as_str() != record.endpoint.as_deref()
        || record.user.is_some_and(|saved| saved != user)
        || record.evaluator.is_some_and(|saved| saved != evaluator)
    {
        return Err(io::Error::other("IdP public seed changed"));
    }
    record.user = Some(user);
    record.evaluator = Some(evaluator);
    control.write_record(&record)?;
    if engine.is_none() {
        if !record.domain_attempted {
            if Path::new(&config.data_dir).exists() {
                return Err(io::Error::other("unattempted Management store exists"));
            }
            record.domain_attempted = true;
            control.write_record(&record)?;
            *engine = Some(crate::open_database(config).await?);
        } else {
            *engine = Some(open_existing(config).await?);
        }
    }
    if command.cancellation.is_cancelled() {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Create cancelled",
        ));
    }
    let opened = engine.as_ref().expect("opened Management");
    let permissions = record
        .permissions
        .iter()
        .copied()
        .zip(ACTIONS)
        .collect::<Vec<_>>();
    crate::provision_initial_administrator(opened.clone(), user, record.role, &permissions).await?;
    crate::provision_initial_device(
        opened.clone(),
        record.device,
        user,
        record.endpoint.as_deref().expect("endpoint"),
        &command.intent.node,
    )
    .await?;
    verify(opened, &record).await?;
    if router.is_none() {
        *router = Some(
            build(
                config,
                control,
                &command.intent,
                &record,
                opened.clone(),
                identity.as_ref().expect("identity").clone(),
            )
            .await?,
        );
    }
    record.complete = true;
    control
        .finish(
            command,
            router.as_ref().expect("Management router").clone(),
            &record,
        )
        .await
}
