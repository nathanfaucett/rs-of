use std::{io, path::Path, sync::Arc};

use api::{
    IndependentCreateCommand, IndependentCreateControl, IndependentCreateIntent,
    IndependentOwnerPublic, IndependentSetupHost,
};
use idp_model::model::Id;
use idp_service::setup_credentials::{SetupRelationship, create_setup_secret, load_setup_secret};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{AppConfig, IdpClient, ManagementClient, RouterState, StorageRuntime};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u8,
    clients: [Id; 2],
    attempted: [bool; 2],
    endpoint: Option<String>,
    domain_attempted: bool,
    complete: bool,
}
impl Record {
    fn new() -> Self {
        Self {
            version: 1,
            clients: [Id::now_v7(), Id::now_v7()],
            attempted: [false; 2],
            endpoint: None,
            domain_attempted: false,
            complete: false,
        }
    }
    fn validate(&self) -> io::Result<()> {
        if self.version != 1
            || self.clients.iter().any(Id::is_nil)
            || self.clients[0] == self.clients[1]
            || (self.complete
                && (!self.attempted.iter().all(|attempted| *attempted)
                    || !self.domain_attempted
                    || self.endpoint.is_none()))
        {
            return Err(io::Error::other(
                "invalid retained Storage service evidence",
            ));
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
    setup.require_owner("storage", Path::new(&config.data_dir))?;
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
    let mut runtime = None;
    if let Some(record) = control.read_record::<Record>()? {
        record.validate()?;
        if record.complete {
            let intent = control
                .intent()
                .await
                .ok_or_else(|| io::Error::other("completed Storage has no exact operation"))?;
            existing(&config)?;
            public(&control, &intent, &record).await?;
            runtime = Some(build(&config, &control, &intent, &record, &shutdown)?);
            control
                .restore(runtime.as_ref().expect("Storage runtime").router())
                .await?;
        }
    }
    loop {
        let command = tokio::select! { biased; () = shutdown.cancelled() => break, command = commands.recv() => match command { Some(command) => command, None => break } };
        if let Err(error) = provision(&config, &control, &command, &mut runtime, &shutdown).await {
            log::warn!(
                "Independent Create owner=storage state=unfinished error_kind={:?}",
                error.kind()
            );
            control.unfinished().await;
        }
    }
    if let Some(runtime) = runtime {
        runtime.shutdown().await?;
    }
    Ok(())
}
fn existing(config: &AppConfig) -> io::Result<()> {
    for child in ["databases", "filesystems"] {
        if !Path::new(&config.data_dir).join(child).is_dir() {
            return Err(io::Error::other("attempted Storage root missing"));
        }
    }
    Ok(())
}
async fn public(
    control: &IndependentCreateControl,
    intent: &IndependentCreateIntent,
    record: &Record,
) -> io::Result<()> {
    let clients = [
        SetupRelationship::StorageIdp,
        SetupRelationship::StorageManagement,
    ]
    .iter()
    .zip(record.clients)
    .map(|(relationship, id)| {
        serde_json::to_value(relationship.registration(
            id,
            &intent.idp,
            &intent.management,
            &intent.storage,
        ))
    })
    .collect::<Result<Vec<_>, _>>()?;
    let scope = service(control);
    let ids = record.clients.map(|id| id.to_string());
    control
        .set_public(
            IndependentOwnerPublic {
                operation: intent.clone(),
                owner: "storage".into(),
                clients,
                facts: serde_json::json!({"endpoint":record.endpoint}),
                registered: Vec::new(),
            },
            Arc::new(move |requested| {
                if !ids.iter().any(|id| id == requested) {
                    return Err(io::Error::other("unknown Storage consumer Client"));
                }
                load_setup_secret(&scope, requested)
            }),
        )
        .await
}
fn build(
    config: &AppConfig,
    control: &IndependentCreateControl,
    intent: &IndependentCreateIntent,
    record: &Record,
    shutdown: &CancellationToken,
) -> io::Result<StorageRuntime> {
    let idp = IdpClient::new(
        &intent.idp,
        &record.clients[0].to_string(),
        &load_setup_secret(&service(control), &record.clients[0].to_string())?,
        &intent.issuer,
        &intent.idp,
    )
    .map_err(io::Error::other)?;
    let management = ManagementClient::new(
        &intent.management,
        &intent.idp,
        &record.clients[1].to_string(),
        &load_setup_secret(&service(control), &record.clients[1].to_string())?,
        &intent.issuer,
        &intent.storage,
    )
    .map_err(io::Error::other)?;
    let endpoint = record
        .endpoint
        .as_deref()
        .ok_or_else(|| io::Error::other("retained Management endpoint missing"))?
        .parse::<iroh::EndpointId>()
        .map_err(io::Error::other)?;
    crate::build_http_runtime(
        RouterState::new(&intent.storage)
            .with_idp_client(idp)
            .with_management_client(management),
        Path::new(&config.data_dir),
        config.server.prefix(),
        endpoint,
        shutdown.child_token(),
    )
}
async fn provision(
    config: &AppConfig,
    control: &IndependentCreateControl,
    command: &IndependentCreateCommand,
    runtime: &mut Option<StorageRuntime>,
    shutdown: &CancellationToken,
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
                runtime
                    .as_ref()
                    .ok_or_else(|| io::Error::other("completed Storage runtime not reopened"))?
                    .router(),
                &record,
            )
            .await;
    }
    for index in 0..2 {
        if !record.attempted[index] {
            record.attempted[index] = true;
            control.write_record(&record)?;
            create_setup_secret(&service(control), &record.clients[index].to_string())?;
        }
        load_setup_secret(&service(control), &record.clients[index].to_string())?;
    }
    public(control, &command.intent, &record).await?;
    for client in record.clients {
        control
            .wait_registered(&client.to_string(), &command.cancellation)
            .await?;
    }
    let management = control
        .wait_peer(&command.intent, "management", &command.cancellation)
        .await?;
    let endpoint = management.facts["endpoint"]
        .as_str()
        .ok_or_else(|| io::Error::other("Management endpoint missing"))?
        .to_owned();
    if endpoint.parse::<iroh::EndpointId>().is_err()
        || record
            .endpoint
            .as_ref()
            .is_some_and(|saved| saved != &endpoint)
    {
        return Err(io::Error::other("Management endpoint changed"));
    }
    record.endpoint = Some(endpoint);
    control.write_record(&record)?;
    if runtime.is_none() {
        if !record.domain_attempted {
            if Path::new(&config.data_dir).exists() {
                return Err(io::Error::other("unattempted Storage root exists"));
            }
            record.domain_attempted = true;
            control.write_record(&record)?;
        } else {
            existing(config)?;
        }
        *runtime = Some(build(config, control, &command.intent, &record, shutdown)?);
    }
    public(control, &command.intent, &record).await?;
    record.complete = true;
    control
        .finish(
            command,
            runtime.as_ref().expect("Storage runtime").router(),
            &record,
        )
        .await
}
