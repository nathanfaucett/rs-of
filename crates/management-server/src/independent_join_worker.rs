use super::{Record, build, open_existing, public, verify};
use std::{io, path::Path, sync::Arc};

use api::{IndependentCreateCommand, IndependentCreateControl, IndependentPeer};
use db::Database;
use idp_model::model::Id;
use iroh_chain::{EndpointIdStore, Server};
use management_service::{
    DeviceIdentity,
    independent_join::{IndependentJoinApproval, IndependentJoinRequest},
    join_history::{JoinHistoryContext, import_join_history},
};

use crate::{AppConfig, IndependentManagementRuntime};

pub(super) async fn provision(
    config: &AppConfig,
    control: &IndependentCreateControl,
    command: &IndependentCreateCommand,
    record: &mut Record,
    engine: &mut Option<Arc<Database>>,
    identity: Arc<DeviceIdentity>,
    runtime: &mut Option<IndependentManagementRuntime>,
) -> io::Result<()> {
    if record.source.is_none() {
        let source = IndependentPeer::discover(
            command
                .intent
                .peer
                .as_deref()
                .ok_or_else(|| io::Error::other("trusted source missing"))?,
        )
        .await?;
        if source.issuer != command.intent.issuer {
            return Err(io::Error::other("authenticated source issuer changed"));
        }
        record.source = Some(source);
        control.write_record(record)?;
    }
    public(control, &command.intent, record).await?;
    if record.approval.is_none() {
        let request = loop {
            if command.cancellation.is_cancelled() {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "Join cancelled"));
            }
            let idp = control
                .wait_peer(&command.intent, "idp", &command.cancellation)
                .await?;
            if !idp.facts["join_request"].is_null() {
                break serde_json::from_value::<IndependentJoinRequest>(
                    idp.facts["join_request"].clone(),
                )?;
            }
            tokio::select! { () = command.cancellation.cancelled() => return Err(io::Error::new(io::ErrorKind::Interrupted,"Join cancelled")), () = tokio::time::sleep(std::time::Duration::from_millis(100)) => () }
        };
        request.validate()?;
        if request.operation != command.intent
            || request.device_id != record.device
            || request.endpoint_id != identity.endpoint_id().to_string()
        {
            return Err(io::Error::other("local Join request binding mismatch"));
        }
        let server = Server::new(identity.endpoint(), EndpointIdStore::default());
        let address: iroh::EndpointAddr = serde_json::from_value(request.source.address.clone())?;
        if address.id.to_string() != request.source.endpoint {
            return Err(io::Error::other("pinned source address mismatch"));
        }
        let payload = serde_json::to_vec(&request)?;
        let reply = tokio::select! { biased; () = command.cancellation.cancelled() => return Err(io::Error::new(io::ErrorKind::Interrupted,"Join cancelled")), result = server.send_pairing_offer(address, &payload) => result? };
        let approval: IndependentJoinApproval = serde_json::from_slice(&reply)
            .map_err(|_| io::Error::other("source Join offer rejected or malformed"))?;
        approval.verify()?;
        if serde_json::to_value(&approval.request)? != serde_json::to_value(&request)? {
            return Err(io::Error::other(
                "approval is not the exact authenticated offer",
            ));
        }
        record.user = Some(approval.owner_subject);
        record.approval = Some(approval);
        control.write_record(record)?;
    }
    let approval = record
        .approval
        .as_ref()
        .expect("retained exact approval")
        .clone();
    approval.verify()?;
    public(control, &command.intent, record).await?;
    if engine.is_none() {
        if !record.domain_attempted {
            if Path::new(&config.data_dir).exists() {
                return Err(io::Error::other("unattempted Management root exists"));
            }
            record.domain_attempted = true;
            control.write_record(record)?;
            *engine = Some(crate::open_database(config).await?);
        } else {
            *engine = Some(open_existing(config).await?);
        }
    }
    let opened = engine.as_ref().expect("opened owner");

    if !record.imported {
        let challenge = control
            .request_history(
                &approval.request.source.management,
                serde_json::to_value(&approval)?,
                &command.cancellation,
            )
            .await?;
        import_join_history(
            opened.clone(),
            JoinHistoryContext::Management,
            &approval,
            challenge["capability"]
                .as_str()
                .ok_or_else(|| io::Error::other("history capability missing"))?,
            control.directory().join("join-history"),
            &command.cancellation,
        )
        .await?;
        verify(opened, record).await?;
        record.imported = true;
        control.write_record(record)?;
    }
    verify(opened, record).await?;
    control
        .wait_registered(&record.client.to_string(), &command.cancellation)
        .await?;
    let idp = control
        .wait_peer(&command.intent, "idp", &command.cancellation)
        .await?;
    let evaluator: Id = serde_json::from_value(idp.facts["evaluator"].clone())?;
    if evaluator.is_nil() || record.evaluator.is_some_and(|saved| saved != evaluator) {
        return Err(io::Error::other("evaluator changed"));
    }
    record.evaluator = Some(evaluator);
    control.write_record(record)?;
    if runtime.is_none() {
        *runtime = Some(
            build(
                config,
                control,
                &command.intent,
                record,
                opened.clone(),
                identity,
            )
            .await?,
        );
    }
    record.complete = true;
    record.validate_operation(&command.intent)?;
    control
        .finish(
            command,
            runtime.as_ref().expect("Management runtime").router(),
            record,
        )
        .await
}
