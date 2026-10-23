use std::{io, path::Path, sync::Arc, time::Instant};

use api::{
    IndependentCreateCommand, IndependentCreateControl, IndependentCreateIntent,
    IndependentOwnerPublic, IndependentSetupHost,
};
use db::Database;
use device_service::DeviceIdentity;
use device_service::DeviceRepo;
use device_service::HostedControlPlane;
use device_service::replica::DbDeviceRepo;
use device_service::replica::DbSelectionPolicyRepo;
use idp_model::{contract::IdentityAction, model::Id};
use idp_service::setup_credentials::{SetupRelationship, create_setup_secret, load_setup_secret};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{AppConfig, IndependentDeviceRuntime};

#[path = "independent_join_worker.rs"]
mod join;

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

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateSeed {
    role: Id,
    permissions: Vec<Id>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u8,
    client: Id,
    device: Id,
    #[serde(skip_serializing_if = "Option::is_none")]
    create: Option<CreateSeed>,
    user: Option<Id>,
    evaluator: Option<Id>,
    secret_attempted: bool,
    endpoint_attempted: bool,
    endpoint: Option<String>,
    domain_attempted: bool,
    source: Option<api::IndependentPeer>,
    approval: Option<device_service::independent_join::IndependentJoinApproval>,
    #[serde(default)]
    imported: bool,
    complete: bool,
}
impl Record {
    fn validate_operation(&self, intent: &IndependentCreateIntent) -> io::Result<()> {
        self.validate()?;
        if intent.purpose == "create" {
            if self.create.is_none()
                || self.source.is_some()
                || self.approval.is_some()
                || self.imported
            {
                return Err(io::Error::other("Create contains Join evidence"));
            }
            return Ok(());
        }
        if self.create.is_some() {
            return Err(io::Error::other(
                "Join contains fresh administrator grant identities",
            ));
        }
        if let Some(source) = &self.source {
            source.verify(
                intent
                    .peer
                    .as_deref()
                    .ok_or_else(|| io::Error::other("Join peer missing"))?,
                &source.nonce,
            )?;
            if source.issuer != intent.issuer {
                return Err(io::Error::other("retained source issuer changed"));
            }
        }
        if let Some(approval) = &self.approval {
            approval.verify()?;
            if approval.request.operation != *intent
                || approval.request.device_id != self.device
                || Some(approval.request.endpoint_id.as_str()) != self.endpoint.as_deref()
                || Some(approval.owner_subject) != self.user
                || self.source.as_ref().map(serde_json::to_value).transpose()?
                    != Some(serde_json::to_value(&approval.request.source)?)
            {
                return Err(io::Error::other(
                    "retained Management Join approval changed",
                ));
            }
        }
        if ((self.complete || self.imported) && (self.source.is_none() || self.approval.is_none()))
            || (self.complete && !self.imported)
        {
            return Err(io::Error::other(
                "completed Management Join lacks exact public import evidence",
            ));
        }
        Ok(())
    }
    fn new(purpose: &str) -> Self {
        Self {
            version: 2,
            client: Id::now_v7(),
            device: Id::now_v7(),
            create: (purpose == "create").then(|| CreateSeed {
                role: Id::now_v7(),
                permissions: ACTIONS.iter().map(|_| Id::now_v7()).collect(),
            }),
            user: None,
            evaluator: None,
            secret_attempted: false,
            endpoint_attempted: false,
            endpoint: None,
            domain_attempted: false,
            source: None,
            approval: None,
            imported: false,
            complete: false,
        }
    }
    fn validate(&self) -> io::Result<()> {
        let mut ids = vec![self.client, self.device];
        if let Some(seed) = &self.create {
            if seed.permissions.len() != ACTIONS.len() {
                return Err(io::Error::other("invalid retained administrator seed"));
            }
            ids.push(seed.role);
            ids.extend(&seed.permissions);
        }
        if self.version != 2
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
    setup.require_owner("device", Path::new(&config.data_dir))?;
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
            record.validate_operation(&intent)?;
            identity = Some(open_identity(&control, &record).await?);
            engine = Some(open_existing(&config).await?);
            verify_identity(engine.as_ref().expect("owner"), &record).await?;
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
            observe(
                &control,
                engine.as_ref().expect("owner").clone(),
                &intent,
                &record,
            )?;
            control
                .restore(router.as_ref().expect("router").router())
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
                "Independent Create owner=device state=unfinished error_kind={:?}",
                error.kind()
            );
            control.unfinished().await;
        }
    }
    if let Some(runtime) = router {
        runtime.shutdown().await?;
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
    if !Path::new(&config.data_dir).join("devices.redb").is_file() {
        return Err(io::Error::other("attempted Management store missing"));
    }
    crate::open_device_database(config).await
}
async fn verify_identity(engine: &Arc<Database>, record: &Record) -> io::Result<String> {
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
    if device.id != record.device || (!record.complete && device.owner_subject != user.to_string())
    {
        return Err(io::Error::other("Management endpoint binding changed"));
    }
    if !device
        .owner_subject
        .parse::<Id>()
        .is_ok_and(|owner| !owner.is_nil())
    {
        return Err(io::Error::other(
            "current Management Device owner is invalid",
        ));
    }
    Ok(device.owner_subject)
}
async fn verify(engine: &Arc<Database>, record: &Record) -> io::Result<()> {
    let owner_subject = verify_identity(engine, record).await?;
    let policies = DbSelectionPolicyRepo::new(engine.clone());
    if policies
        .local_cleanup_pending(record.device)
        .await
        .map_err(io::Error::other)?
        || !policies
            .get(record.device, &owner_subject, Id::nil())
            .await
            .map_err(io::Error::other)?
            .is_some_and(|policy| policy.admin_allowed)
    {
        return Err(io::Error::other("current local endpoint policy missing"));
    }
    Ok(())
}
fn observe(
    control: &IndependentCreateControl,
    engine: Arc<Database>,
    intent: &IndependentCreateIntent,
    record: &Record,
) -> io::Result<()> {
    let intent = intent.clone();
    let record = record.clone();
    let expected = serde_json::to_value(&record)?;
    let secret = load_setup_secret(&service(control), &record.client.to_string())?;
    control.set_readiness(move |control| {
        let (engine, intent, record, expected, secret) = (
            engine.clone(),
            intent.clone(),
            record.clone(),
            expected.clone(),
            secret.clone(),
        );
        async move {
            let current: Record = control
                .read_record()?
                .ok_or_else(|| io::Error::other("Management worker evidence missing"))?;
            current.validate_operation(&intent)?;
            let endpoint: iroh::SecretKey = load_setup_secret(&service(&control), "endpoint")?
                .parse()
                .map_err(io::Error::other)?;
            if serde_json::to_value(current)? != expected
                || record.endpoint.as_deref() != Some(endpoint.public().to_string().as_str())
                || load_setup_secret(&service(&control), &record.client.to_string())? != secret
            {
                return Err(io::Error::other(
                    "retained Management setup evidence changed",
                ));
            }
            verify(&engine, &record).await
        }
    })
}

async fn public(
    control: &IndependentCreateControl,
    intent: &IndependentCreateIntent,
    record: &Record,
) -> io::Result<()> {
    let client = SetupRelationship::ManagementIdp.registration(
        record.client,
        &intent.idp,
        &intent.device,
        &intent.storage,
    );
    let scope = service(control);
    let id = record.client.to_string();
    control
        .set_public(
            IndependentOwnerPublic {
                operation: intent.clone(),
                owner: "device".into(),
                clients: vec![serde_json::to_value(client)?],
                facts: serde_json::json!({"endpoint":record.endpoint, "device":record.device, "source":record.source, "approval":record.approval}),
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
) -> io::Result<IndependentDeviceRuntime> {
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
        .with_device_identity(identity.clone());
    crate::build_independent_device(
        config,
        engine,
        Arc::new(plane),
        identity,
        &intent.issuer,
        &intent.idp,
        &intent.device,
        &intent.storage,
        control.shutdown_token(),
    )
    .await
}
async fn provision(
    config: &AppConfig,
    control: &IndependentCreateControl,
    command: &IndependentCreateCommand,
    engine: &mut Option<Arc<Database>>,
    identity: &mut Option<Arc<DeviceIdentity>>,
    router: &mut Option<IndependentDeviceRuntime>,
) -> io::Result<()> {
    let mut record = match control.read_record::<Record>()? {
        Some(record) => record,
        None => {
            let record = Record::new(&command.intent.purpose);
            control.write_record(&record)?;
            record
        }
    };
    record.validate_operation(&command.intent)?;
    if record.complete {
        return control
            .finish(
                command,
                router
                    .as_ref()
                    .ok_or_else(|| io::Error::other("completed Management runtime not reopened"))?
                    .router(),
                &record,
            )
            .await;
    }
    let protected_at = Instant::now();
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
    log::debug!(
        "Independent setup owner=device purpose={} phase=consumer-endpoint-keys elapsed_ms={}",
        command.intent.purpose,
        protected_at.elapsed().as_millis()
    );
    if command.intent.purpose == "join" {
        return join::provision(
            config,
            control,
            command,
            &mut record,
            engine,
            identity.as_ref().expect("identity").clone(),
            router,
        )
        .await;
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
    let opened_at = Instant::now();
    if engine.is_none() {
        if !record.domain_attempted {
            if Path::new(&config.data_dir).exists() {
                return Err(io::Error::other("unattempted Management store exists"));
            }
            record.domain_attempted = true;
            control.write_record(&record)?;
            *engine = Some(crate::open_device_database(config).await?);
        } else {
            *engine = Some(open_existing(config).await?);
        }
    }
    log::debug!(
        "Independent setup owner=device purpose=create phase=schema-open elapsed_ms={}",
        opened_at.elapsed().as_millis()
    );
    if command.cancellation.is_cancelled() {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Create cancelled",
        ));
    }
    let opened = engine.as_ref().expect("opened Management");
    let seeded_at = Instant::now();
    let seed = record.create.as_ref().expect("validated Create seed");
    let permissions = seed
        .permissions
        .iter()
        .copied()
        .zip(ACTIONS)
        .collect::<Vec<_>>();
    management_server::provision_initial_administrator(
        crate::provisioning::open_identity_database(config)?,
        user,
        seed.role,
        &permissions,
    )
    .await?;
    crate::provision_initial_device(
        opened.clone(),
        record.device,
        user,
        record.endpoint.as_deref().expect("endpoint"),
        &command.intent.node,
    )
    .await?;
    verify(opened, &record).await?;
    log::debug!(
        "Independent setup owner=device purpose=create phase=semantic-seed elapsed_ms={}",
        seeded_at.elapsed().as_millis()
    );
    let serving_at = Instant::now();
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
    log::debug!(
        "Independent setup owner=device purpose=create phase=serving elapsed_ms={}",
        serving_at.elapsed().as_millis()
    );
    record.complete = true;
    observe(control, opened.clone(), &command.intent, &record)?;
    control
        .finish(
            command,
            router.as_ref().expect("Management router").router(),
            &record,
        )
        .await
}
