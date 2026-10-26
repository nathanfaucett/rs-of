use std::{io, path::Path, sync::Arc, time::Instant};

use api::{IndependentCreateCommand, IndependentCreateControl};
use device_service::independent_join::IndependentJoinApproval;
use device_service::independent_join::IndependentJoinRequest;
use device_service::join_history::JoinHistoryContext;
use device_service::join_history::import_join_history;
use idp_model::{contract::ClientRegistration, model::Id};
use idp_service::setup_credentials::{SetupRelationship, load_setup_secret};

use super::{Record, binding, login, observe, open, public, runtime_config, service};
use crate::{AppConfig, IdpRuntime, OwnerProvisioner};

pub(super) async fn provision(
    config: &AppConfig,
    control: &IndependentCreateControl,
    command: &IndependentCreateCommand,
    record: &mut Record,
    owner: &mut Option<Arc<OwnerProvisioner>>,
    runtime: &mut Option<IdpRuntime>,
    owners: &tokio::sync::watch::Sender<Option<Arc<db::Database>>>,
) -> io::Result<()> {
    let intent = &command.intent;
    let device = control
        .wait_peer(intent, "device", &command.cancellation)
        .await?;
    let storage = control
        .wait_peer(intent, "storage", &command.cancellation)
        .await?;
    let endpoint = device.facts["endpoint"]
        .as_str()
        .ok_or_else(|| io::Error::other("Management endpoint missing"))?
        .to_owned();
    if record
        .endpoint
        .as_ref()
        .is_some_and(|saved| saved != &endpoint)
    {
        return Err(io::Error::other("Management endpoint changed"));
    }
    record.endpoint = Some(endpoint.clone());
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
            return Err(io::Error::other("consumer relationships mismatch"));
        }
        for (value, relationship) in peer.clients.iter().zip(relationships) {
            let registration: ClientRegistration = serde_json::from_value(value.clone())?;
            let id: Id = registration
                .client_id
                .as_deref()
                .ok_or_else(|| io::Error::other("Client ID missing"))?
                .parse()
                .map_err(io::Error::other)?;
            if id.is_nil()
                || registration
                    != relationship.registration(id, &intent.idp, &intent.device, &intent.storage)
            {
                return Err(io::Error::other("least-privilege consumer binding changed"));
            }
            registrations.push(registration);
        }
    }
    if !record.registrations.is_empty() && record.registrations != registrations {
        return Err(io::Error::other("retained consumer bindings changed"));
    }
    record.registrations = registrations;
    control.write_record(record)?;
    let opened_at = Instant::now();
    if owner.is_none() {
        if !record.domain_attempted {
            if Path::new(&config.data_dir).exists() {
                return Err(io::Error::other("unattempted IdP root exists"));
            }
            record.domain_attempted = true;
            control.write_record(record)?;
            *owner = Some(Arc::new(
                OwnerProvisioner::open(&runtime_config(config, intent, record)?).await?,
            ));
        } else {
            open(config, intent, record, owner).await?;
        }
    }
    log::debug!(
        "Independent setup owner=idp purpose=join phase=schema-open elapsed_ms={}",
        opened_at.elapsed().as_millis()
    );
    owners.send_replace(owner.as_ref().expect("opened own IdP").database.clone());
    let opened = owner.as_ref().expect("opened own IdP");
    let protected_at = Instant::now();
    let mut clients = vec![record.login, record.client];
    for registration in &record.registrations {
        clients.push(
            registration
                .client_id
                .as_deref()
                .expect("validated Client ID")
                .parse()
                .map_err(io::Error::other)?,
        );
    }
    if !record.inventory_attempted {
        record.inventory_attempted = true;
        control.write_record(record)?;
        opened
            .prepare_fresh_setup_secret_inventory(record.user, &clients)
            .await?;
    }
    opened.prepare_setup_client_slots(&clients).await?;
    if command.cancellation.is_cancelled() {
        return Err(io::Error::new(io::ErrorKind::Interrupted, "Join cancelled"));
    }
    if record.request.is_none() {
        let source = serde_json::from_value(device.facts["source"].clone())?;
        let device_id = serde_json::from_value(device.facts["device"].clone())?;
        let request = IndependentJoinRequest {
            operation: intent.clone(),
            device_id,
            member_id: record.member,
            endpoint_id: endpoint,
            public_signer: opened.prepare_node_signer(&binding(record)?)?,
            source,
        };
        request.validate()?;
        record.request = Some(request);
        control.write_record(record)?;
    }
    let request = record
        .request
        .as_ref()
        .expect("retained exact request")
        .clone();
    request.validate()?;
    if request.operation != *intent
        || request.member_id != record.member
        || request.endpoint_id != record.endpoint.as_deref().expect("endpoint")
        || opened.verify_prepared_node_signer(&binding(record)?)? != request.public_signer
    {
        return Err(io::Error::other("protected Join signer changed"));
    }
    log::debug!(
        "Independent setup owner=idp purpose=join phase=protected-signer elapsed_ms={}",
        protected_at.elapsed().as_millis()
    );
    public(config, control, intent, record).await?;
    let approval_at = Instant::now();
    if record.approval.is_none() {
        let approval = loop {
            let peer = control
                .wait_peer(intent, "device", &command.cancellation)
                .await?;
            if !peer.facts["approval"].is_null() {
                break serde_json::from_value::<IndependentJoinApproval>(
                    peer.facts["approval"].clone(),
                )?;
            }
            tokio::select! { () = command.cancellation.cancelled() => return Err(io::Error::new(io::ErrorKind::Interrupted,"Join cancelled")), () = tokio::time::sleep(std::time::Duration::from_millis(100)) => () }
        };
        approval.verify()?;
        if serde_json::to_value(&approval.request)? != serde_json::to_value(&request)? {
            return Err(io::Error::other(
                "approval does not bind this exact request",
            ));
        }
        record.approval = Some(approval);
        control.write_record(record)?;
    }
    log::debug!(
        "Independent setup owner=idp purpose=join phase=approval-wait elapsed_ms={}",
        approval_at.elapsed().as_millis()
    );
    let approval = record.approval.as_ref().expect("retained approval").clone();
    approval.verify()?;

    let import_at = Instant::now();
    if !record.imported {
        if config.identity_database.is_some() {
            approval.verify_current("idp").await?;
        } else {
            log::debug!("Join history owner=idp phase=challenge_start");
            let challenge = control
                .request_history(
                    &approval.request.source.idp,
                    serde_json::to_value(&approval)?,
                    &command.cancellation,
                )
                .await;
            log::debug!(
                "Join history owner=idp phase=challenge_end elapsed_ms={} error_kind={:?}",
                import_at.elapsed().as_millis(),
                challenge.as_ref().err().map(io::Error::kind)
            );
            let challenge = challenge?;

            log::debug!("Join history owner=idp phase=import_start");
            let imported = import_join_history(
                opened.node_join_database()?,
                JoinHistoryContext::Idp,
                &approval,
                challenge["capability"]
                    .as_str()
                    .ok_or_else(|| io::Error::other("history capability missing"))?,
                control.directory().join("join-history"),
                &command.cancellation,
            )
            .await;
            log::debug!(
                "Join history owner=idp phase=import_end elapsed_ms={} error_kind={:?}",
                import_at.elapsed().as_millis(),
                imported.as_ref().err().map(io::Error::kind)
            );
            imported?;
        }
        log::debug!("Join history owner=idp phase=import_signer_start");
        let verified = opened.verify_node_signer(&binding(record)?).await;
        log::debug!(
            "Join history owner=idp phase=import_signer_end elapsed_ms={} error_kind={:?}",
            import_at.elapsed().as_millis(),
            verified.as_ref().err().map(io::Error::kind)
        );
        verified?;
        record.imported = true;
        record.user = approval.owner_subject;
        control.write_record(record)?;
    }
    log::debug!("Join history owner=idp phase=retained_signer_start");
    let verified = opened.verify_node_signer(&binding(record)?).await;
    log::debug!(
        "Join history owner=idp phase=retained_signer_end elapsed_ms={} error_kind={:?}",
        import_at.elapsed().as_millis(),
        verified.as_ref().err().map(io::Error::kind)
    );
    verified?;
    log::debug!(
        "Independent setup owner=idp purpose=join phase=public-import elapsed_ms={}",
        import_at.elapsed().as_millis()
    );
    let registered_at = Instant::now();

    opened
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
    opened
        .ensure_setup_infrastructure_client(own_registration)
        .await?;
    let registrar = opened.clone();
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
    public(config, control, intent, record).await?;
    log::debug!(
        "Independent setup owner=idp purpose=join phase=local-client-seed elapsed_ms={}",
        registered_at.elapsed().as_millis()
    );
    let serving_at = Instant::now();
    if runtime.is_none() {
        *runtime = Some(
            crate::build_independent_idp(
                &runtime_config(config, intent, record)?,
                opened.engine.clone(),
                opened.database.clone(),
                control.shutdown_token(),
            )
            .await?,
        );
    }
    control
        .publish_serving(runtime.as_ref().expect("IdP runtime").router())
        .await?;
    log::debug!(
        "Independent setup owner=idp purpose=join phase=serving elapsed_ms={}",
        serving_at.elapsed().as_millis()
    );
    let relationships_at = Instant::now();
    for peer in [&device, &storage] {
        for registration in &peer.clients {
            if command.cancellation.is_cancelled() {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "Join cancelled"));
            }
            if control.registration_confirmed(intent, registration)?
                && peer
                    .registered
                    .iter()
                    .any(|id| Some(id.as_str()) == registration["client_id"].as_str())
            {
                opened
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
        "Independent setup owner=idp purpose=join phase=relationship-registration elapsed_ms={}",
        relationships_at.elapsed().as_millis()
    );
    record.complete = true;
    record.validate_operation(&command.intent)?;
    observe(control, opened.clone(), intent, record)?;
    control
        .finish(
            command,
            runtime.as_ref().expect("IdP runtime").router(),
            record,
        )
        .await
}
