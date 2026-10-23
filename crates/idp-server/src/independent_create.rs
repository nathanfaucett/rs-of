use std::{io, path::Path, sync::Arc, time::Instant};

use api::{
    IndependentCreateCommand, IndependentCreateControl, IndependentCreateIntent,
    IndependentOwnerPublic, IndependentSetupHost,
};
use idp_model::{
    contract::{
        ApplicationRegistration, ClientProfile, ClientRegistration, ClientType, GrantType,
        TokenEndpointAuthMethod,
    },
    model::Id,
};
use idp_service::{
    oauth2::NodeSignerBinding,
    setup_credentials::{SetupRelationship, create_setup_secret, load_setup_secret},
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{AppConfig, IdpRuntime, ManagementPermissionConfig, OwnerProvisioner};

#[path = "independent_join_worker.rs"]
mod join;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u8,
    user: Id,
    #[serde(skip_serializing_if = "Option::is_none")]
    credential: Option<Id>,
    login: Id,
    member: Id,
    key: Id,
    client: Id,
    secret_attempted: bool,
    domain_attempted: bool,
    inventory_attempted: bool,
    endpoint: Option<String>,
    registrations: Vec<ClientRegistration>,
    request: Option<device_service::independent_join::IndependentJoinRequest>,
    approval: Option<device_service::independent_join::IndependentJoinApproval>,
    #[serde(default)]
    imported: bool,
    complete: bool,
}

impl Record {
    fn validate_operation(&self, intent: &IndependentCreateIntent) -> io::Result<()> {
        self.validate()?;
        if intent.purpose == "create" {
            if self.credential.is_none()
                || self.request.is_some()
                || self.approval.is_some()
                || self.imported
            {
                return Err(io::Error::other("Create contains Join evidence"));
            }
            return Ok(());
        }
        if self.credential.is_some() {
            return Err(io::Error::other(
                "Join contains a fresh credential identity",
            ));
        }
        if let Some(request) = &self.request {
            request.validate()?;
            if request.operation != *intent
                || request.member_id != self.member
                || request.public_signer.kid != self.key.to_string()
                || Some(request.endpoint_id.as_str()) != self.endpoint.as_deref()
            {
                return Err(io::Error::other("retained Join signer binding changed"));
            }
        }
        if let Some(approval) = &self.approval {
            approval.verify()?;
            if self
                .request
                .as_ref()
                .map(serde_json::to_value)
                .transpose()?
                != Some(serde_json::to_value(&approval.request)?)
                || (self.imported && approval.owner_subject != self.user)
            {
                return Err(io::Error::other("retained Join approval changed"));
            }
        }
        if ((self.complete || self.imported) && (self.request.is_none() || self.approval.is_none()))
            || (self.complete && !self.imported)
        {
            return Err(io::Error::other(
                "completed Join lacks exact public import evidence",
            ));
        }
        Ok(())
    }
    fn new(purpose: &str) -> Self {
        Self {
            version: 2,
            user: Id::now_v7(),
            credential: (purpose == "create").then(Id::now_v7),
            login: Id::now_v7(),
            member: Id::now_v7(),
            key: Id::now_v7(),
            client: Id::now_v7(),
            secret_attempted: false,
            domain_attempted: false,
            inventory_attempted: false,
            endpoint: None,
            registrations: Vec::new(),
            request: None,
            approval: None,
            imported: false,
            complete: false,
        }
    }
    fn validate(&self) -> io::Result<()> {
        let mut ids = vec![self.user, self.login, self.member, self.key, self.client];
        ids.extend(self.credential);
        if self.version != 2
            || ids.iter().any(Id::is_nil)
            || ids.iter().enumerate().any(|(i, id)| ids[..i].contains(id))
            || self.registrations.iter().any(|r| r.client_secret.is_some())
            || (self.complete
                && (!self.domain_attempted
                    || !self.inventory_attempted
                    || !self.secret_attempted
                    || self.endpoint.is_none()
                    || self.registrations.len() != 3))
        {
            return Err(io::Error::other(
                "invalid retained independent IdP evidence",
            ));
        }
        Ok(())
    }
}

pub async fn serve_independent_create(
    config: Arc<AppConfig>,
    setup: IndependentSetupHost,
    shutdown: CancellationToken,
) -> io::Result<()> {
    if config.identity_database.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "remote identity consumers attach to an initialized Installation; independent Create/Join requires a local database owner",
        ));
    }
    setup.require_owner("idp", Path::new(&config.data_dir))?;
    let (host, commands) = setup.into_create(shutdown.clone())?;
    let control = host.control();
    let mut worker = tokio::spawn(run(config, control, commands, shutdown.clone()));
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
    let mut owner = None;
    let mut runtime = None;
    if let Some(record) = control.read_record::<Record>()? {
        record.validate()?;
        if record.complete {
            let intent = control
                .intent()
                .await
                .ok_or_else(|| io::Error::other("completed IdP has no exact operation"))?;
            record.validate_operation(&intent)?;
            open(&config, &control, &intent, &record, &mut owner).await?;
            let opened = owner.as_ref().expect("opened IdP");
            opened.verify_node_signer(&binding(&record)?).await?;
            opened
                .verify_public_login_client(login(&intent, record.login))
                .await?;
            opened
                .verify_setup_infrastructure_registration(
                    &SetupRelationship::IdpManagement.registration(
                        record.client,
                        &intent.idp,
                        &intent.device,
                        &intent.storage,
                    ),
                )
                .await?;
            for registration in &record.registrations {
                opened
                    .verify_setup_infrastructure_registration(registration)
                    .await?;
            }
            public(&control, &intent, &record).await?;
            runtime = Some(
                crate::build_independent_idp(
                    &runtime_config(&config, &control, &intent, &record)?,
                    opened.node_join_database()?,
                    control.shutdown_token(),
                )
                .await?,
            );
            observe(&control, opened.clone(), &intent, &record)?;
            control
                .restore(runtime.as_ref().expect("restored runtime").router())
                .await?;
        }
    }
    loop {
        let command = tokio::select! { biased; () = shutdown.cancelled() => break, command = commands.recv() => match command { Some(command) => command, None => break } };
        if let Err(error) = provision(&config, &control, &command, &mut owner, &mut runtime).await {
            log::warn!(
                "Independent Create owner=idp state=unfinished error_kind={:?}",
                error.kind()
            );
            control.unfinished().await;
        }
    }
    drop(runtime);
    drop(owner);
    Ok(())
}

fn service(control: &IndependentCreateControl) -> String {
    format!("independent-setup:{}", control.directory().display())
}
fn binding(record: &Record) -> io::Result<NodeSignerBinding> {
    Ok(NodeSignerBinding {
        member_id: record.member,
        key_id: record.key,
        endpoint_id: record
            .endpoint
            .clone()
            .ok_or_else(|| io::Error::other("retained Management endpoint missing"))?,
    })
}
fn runtime_config(
    config: &AppConfig,
    control: &IndependentCreateControl,
    intent: &IndependentCreateIntent,
    record: &Record,
) -> io::Result<AppConfig> {
    let mut config = config.clone();
    config.api_public_uri = intent.idp.clone();
    config.oauth2.issuer = intent.issuer.clone();
    config.oauth2.local_signer = Some(binding(record)?);
    config.service_audience = Some(intent.idp.clone());
    config.management_data_dir = None;
    config.control_plane_uri = None;
    config.management_permissions = Some(ManagementPermissionConfig {
        api_base_uri: intent.device.clone(),
        client_id: record.client.to_string(),
        client_secret: load_setup_secret(&service(control), &record.client.to_string())?,
    });
    Ok(config)
}
async fn open(
    config: &AppConfig,
    control: &IndependentCreateControl,
    intent: &IndependentCreateIntent,
    record: &Record,
    opened: &mut Option<Arc<OwnerProvisioner>>,
) -> io::Result<()> {
    if opened.is_none() {
        if !Path::new(&config.data_dir).join("idp.redb").is_file() {
            return Err(io::Error::other("attempted IdP store is missing"));
        }
        *opened = Some(Arc::new(
            OwnerProvisioner::open(&runtime_config(config, control, intent, record)?).await?,
        ));
    }
    Ok(())
}
fn observe(
    control: &IndependentCreateControl,
    owner: Arc<OwnerProvisioner>,
    intent: &IndependentCreateIntent,
    record: &Record,
) -> io::Result<()> {
    let intent = intent.clone();
    let record = record.clone();
    let expected = serde_json::to_value(&record)?;
    let secret = load_setup_secret(&service(control), &record.client.to_string())?;
    control.set_readiness(move |control| {
        let (owner, intent, record, expected, secret) = (
            owner.clone(),
            intent.clone(),
            record.clone(),
            expected.clone(),
            secret.clone(),
        );
        async move {
            let current: Record = control
                .read_record()?
                .ok_or_else(|| io::Error::other("IdP worker evidence missing"))?;
            current.validate_operation(&intent)?;
            if serde_json::to_value(current)? != expected
                || load_setup_secret(&service(&control), &record.client.to_string())? != secret
            {
                return Err(io::Error::other("retained IdP setup evidence changed"));
            }
            owner.verify_node_signer(&binding(&record)?).await?;
            owner
                .verify_public_login_client(login(&intent, record.login))
                .await?;
            owner
                .oauth2
                .find_user_info(record.user)
                .await
                .map_err(|_| io::Error::other("current setup owner is inactive"))?;
            owner
                .verify_setup_infrastructure_registration(
                    &SetupRelationship::IdpManagement.registration(
                        record.client,
                        &intent.idp,
                        &intent.device,
                        &intent.storage,
                    ),
                )
                .await?;
            for registration in &record.registrations {
                owner
                    .verify_setup_infrastructure_registration(registration)
                    .await?;
            }
            Ok(())
        }
    })
}

async fn public(
    control: &IndependentCreateControl,
    intent: &IndependentCreateIntent,
    record: &Record,
) -> io::Result<()> {
    let registration = SetupRelationship::IdpManagement.registration(
        record.client,
        &intent.idp,
        &intent.device,
        &intent.storage,
    );
    let scope = service(control);
    let id = record.client.to_string();
    control.set_public(IndependentOwnerPublic { operation: intent.clone(), owner: "idp".into(), clients: vec![serde_json::to_value(registration)?], facts: serde_json::json!({"user":record.user, "login_client":record.login, "endpoint":record.endpoint, "evaluator":record.client, "join_request":record.request, "approval":record.approval}), registered: Vec::new() }, Arc::new(move |requested| {
        if requested != id { return Err(io::Error::other("unknown consumer Client")); }
        load_setup_secret(&scope, requested)
    })).await
}

async fn provision(
    config: &AppConfig,
    control: &IndependentCreateControl,
    command: &IndependentCreateCommand,
    owner: &mut Option<Arc<OwnerProvisioner>>,
    runtime: &mut Option<IdpRuntime>,
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
    let intent = &command.intent;
    if record.complete {
        let current = runtime
            .as_ref()
            .ok_or_else(|| io::Error::other("completed IdP runtime not reopened"))?;
        return control.finish(command, current.router(), &record).await;
    }
    let protected = Instant::now();
    if !record.secret_attempted {
        record.secret_attempted = true;
        control.write_record(&record)?;
        create_setup_secret(&service(control), &record.client.to_string())?;
    }
    load_setup_secret(&service(control), &record.client.to_string())?;
    log::debug!(
        "Independent setup owner=idp purpose={} phase=consumer-key elapsed_ms={}",
        intent.purpose,
        protected.elapsed().as_millis()
    );
    public(control, intent, &record).await?;
    if intent.purpose == "join" {
        return join::provision(config, control, command, &mut record, owner, runtime).await;
    }
    let device = control
        .wait_peer(intent, "device", &command.cancellation)
        .await?;
    let storage = control
        .wait_peer(intent, "storage", &command.cancellation)
        .await?;
    let endpoint = device.facts["endpoint"]
        .as_str()
        .ok_or_else(|| io::Error::other("Management endpoint is missing"))?
        .to_owned();
    if endpoint.parse::<iroh::EndpointId>().is_err()
        || record
            .endpoint
            .as_ref()
            .is_some_and(|saved| saved != &endpoint)
    {
        return Err(io::Error::other("Management endpoint changed"));
    }
    let mut registrations = Vec::new();
    for (peer, relationships) in [
        (&device, &[SetupRelationship::ManagementIdp][..]),
        (
            &storage,
            &[
                SetupRelationship::StorageIdp,
                SetupRelationship::StorageManagement,
            ][..],
        ),
    ] {
        if peer.clients.len() != relationships.len() {
            return Err(io::Error::other("unexpected consumer relationships"));
        }
        for (value, relationship) in peer.clients.iter().zip(relationships) {
            let registration: ClientRegistration = serde_json::from_value(value.clone())?;
            let id: Id = registration
                .client_id
                .as_deref()
                .ok_or_else(|| io::Error::other("consumer Client ID missing"))?
                .parse()
                .map_err(io::Error::other)?;
            if id.is_nil()
                || registration
                    != relationship.registration(id, &intent.idp, &intent.device, &intent.storage)
            {
                return Err(io::Error::other(
                    "consumer least-privilege registration changed",
                ));
            }
            registrations.push(registration);
        }
    }
    if !record.registrations.is_empty() && record.registrations != registrations {
        return Err(io::Error::other("retained consumer Clients changed"));
    }
    record.registrations = registrations;
    record.endpoint = Some(endpoint);
    control.write_record(&record)?;
    let mut ids = vec![record.login, record.client];
    for registration in &record.registrations {
        ids.push(
            registration
                .client_id
                .as_ref()
                .expect("validated Client ID")
                .parse()
                .map_err(io::Error::other)?,
        );
    }
    let opened_at = Instant::now();
    if owner.is_none() {
        if !record.domain_attempted {
            if Path::new(&config.data_dir).exists() {
                return Err(io::Error::other("unattempted IdP store exists"));
            }
            record.domain_attempted = true;
            control.write_record(&record)?;
            *owner = Some(Arc::new(
                OwnerProvisioner::open(&runtime_config(config, control, intent, &record)?).await?,
            ));
        } else {
            open(config, control, intent, &record, owner).await?;
        }
    }
    log::debug!(
        "Independent setup owner=idp purpose=create phase=schema-open elapsed_ms={}",
        opened_at.elapsed().as_millis()
    );
    let owner = owner.as_ref().expect("opened IdP owner");
    let inventory_at = Instant::now();
    if !record.inventory_attempted {
        record.inventory_attempted = true;
        control.write_record(&record)?;
        owner
            .prepare_fresh_setup_secret_inventory(record.user, &ids)
            .await?;
    }
    owner.prepare_setup_client_slots(&ids).await?;
    log::debug!(
        "Independent setup owner=idp purpose=create phase=protected-inventory elapsed_ms={}",
        inventory_at.elapsed().as_millis()
    );
    if command.cancellation.is_cancelled() {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Create cancelled",
        ));
    }
    let password_at = Instant::now();
    owner
        .ensure_initial_user(
            record.user,
            record.credential.expect("validated Create credential"),
            &intent.administrator,
            command
                .password
                .as_deref()
                .ok_or_else(|| io::Error::other("protected administrator input missing"))?,
        )
        .await?;
    log::debug!(
        "Independent setup owner=idp purpose=create phase=password-credential elapsed_ms={}",
        password_at.elapsed().as_millis()
    );
    let signing_at = Instant::now();
    owner.ensure_initial_user_root(record.user).await?;
    owner
        .ensure_initial_node(&binding(&record)?, record.user, &intent.node)
        .await?;
    log::debug!(
        "Independent setup owner=idp purpose=create phase=signing-key-root-seed elapsed_ms={}",
        signing_at.elapsed().as_millis()
    );
    let clients_at = Instant::now();
    owner
        .ensure_setup_public_login_client(login(intent, record.login))
        .await?;
    let mut own_registration = SetupRelationship::IdpManagement.registration(
        record.client,
        &intent.idp,
        &intent.device,
        &intent.storage,
    );
    own_registration.client_secret = Some(load_setup_secret(
        &service(control),
        &record.client.to_string(),
    )?);
    owner
        .ensure_setup_infrastructure_client(own_registration)
        .await?;
    log::debug!(
        "Independent setup owner=idp purpose=create phase=client-registration-seed elapsed_ms={}",
        clients_at.elapsed().as_millis()
    );
    public(control, intent, &record).await?;
    let registrar = Arc::clone(owner);
    control
        .set_registrar(Arc::new(move |registration| {
            let registrar = registrar.clone();
            Box::pin(async move {
                registrar
                    .ensure_setup_infrastructure_client(serde_json::from_value(registration)?)
                    .await?;
                Ok(())
            })
        }))
        .await;
    let serving_at = Instant::now();
    if runtime.is_none() {
        *runtime = Some(
            crate::build_independent_idp(
                &runtime_config(config, control, intent, &record)?,
                owner.node_join_database()?,
                control.shutdown_token(),
            )
            .await?,
        );
    }
    control
        .publish_serving(runtime.as_ref().expect("IdP runtime").router())
        .await?;
    log::debug!(
        "Independent setup owner=idp purpose=create phase=serving elapsed_ms={}",
        serving_at.elapsed().as_millis()
    );
    let relationships_at = Instant::now();
    for peer in [&device, &storage] {
        for registration in &peer.clients {
            if command.cancellation.is_cancelled() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Create cancelled",
                ));
            }
            if control.registration_confirmed(intent, registration)?
                && peer
                    .registered
                    .iter()
                    .any(|id| Some(id.as_str()) == registration["client_id"].as_str())
            {
                owner
                    .verify_setup_infrastructure_registration(&serde_json::from_value(
                        registration.clone(),
                    )?)
                    .await?;
            } else {
                control.register_recipient(peer, registration).await?;
            }
        }
    }
    log::debug!(
        "Independent setup owner=idp purpose=create phase=relationship-registration elapsed_ms={}",
        relationships_at.elapsed().as_millis()
    );
    record.complete = true;
    observe(control, owner.clone(), intent, &record)?;
    control
        .finish(
            command,
            runtime.as_ref().expect("IdP runtime").router(),
            &record,
        )
        .await
}

fn login(intent: &IndependentCreateIntent, id: Id) -> ClientRegistration {
    ClientRegistration {
        application: ApplicationRegistration {
            name: Some("Local Node operator".into()),
            uri: format!("{}/operator", intent.idp),
            description: None,
        },
        client_id: Some(id.to_string()),
        client_secret: None,
        client_id_issued_at: None,
        client_secret_expires_at: None,
        client_name: "Local Node operator".into(),
        client_uri: None,
        logo_uri: None,
        contacts: Vec::new(),
        terms_of_service_uri: None,
        policy_uri: None,
        client_type: ClientType::Public,
        profile: ClientProfile::Native,
        redirect_uris: Vec::new(),
        allowed_grant_types: vec![
            GrantType::Password,
            GrantType::RefreshToken,
            GrantType::TokenExchange,
        ],
        response_types: Vec::new(),
        allowed_scopes: vec!["openid".into(), "profile".into(), "email".into()],
        allowed_audiences: vec![
            intent.idp.clone(),
            "idp-management".into(),
            intent.storage.clone(),
            "storage:databases".into(),
            "storage:filesystems".into(),
        ],
        token_endpoint_auth_method: TokenEndpointAuthMethod::None,
        software_statement: None,
        software_id: None,
        software_version: None,
    }
}
