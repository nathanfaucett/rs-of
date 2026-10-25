use std::{fs, time::Duration};

use of_idp_client::{
    apis::{configuration::Configuration, default_api as idp},
    models,
};
use of_storage_client::apis::resources_api as storage;
use serde_json::json;

use super::{Node, device_config, use_node};

pub async fn denial_scenario(independent_layout: bool) {
    use of_management_client::apis::default_api as policy;
    let mut node = Node::new();
    if independent_layout {
        node.independent_create(true).await;
    } else {
        node.unified().await;
    }
    let resource = use_node(&mut node, None, None).await;
    let session = node.login().await;
    let mut user = node.config(&node.idp);
    user.bearer_access_token = Some(session.clone());
    let subject = idp::userinfo(&user)
        .await
        .unwrap_or_else(|_| panic!("public User info"))
        .sub;
    let authority = node.exchange(&session, "idp-management", "").await;
    let config = of_management_client::apis::configuration::Configuration {
        client: node.http.clone(),
        base_path: node.management.clone(),
        bearer_access_token: Some(authority.clone()),
        ..Default::default()
    };
    let mut device = device_config(&node);
    device.bearer_access_token = Some(authority);
    let local = of_device_client::apis::default_api::list_devices(&device)
        .await
        .unwrap_or_else(|_| panic!("public Device list"))
        .into_iter()
        .next()
        .expect("created Node Device");
    let request: of_management_client::models::PermissionEvaluationRequest = serde_json::from_value(json!({
        "requestId":"01234567-89ab-4cde-8fab-0123456789ab", "action":"devices_read",
        "subject":{"type":"user","id":subject}, "target":{"scope":"installation","resource":{"type":"device","id":local.id}}
    })).expect("generated permission request");
    let decision = policy::evaluate_permission(
        &config,
        policy::EvaluatePermissionParams {
            permission_evaluation_request: request.clone(),
        },
    )
    .await
    .unwrap_or_else(|_| panic!("authorized exact evaluation"));
    assert!(decision.allowed);
    assert_eq!(*decision.request, request);
    assert_eq!(*decision.audit.actor, *request.subject);
    assert_eq!(decision.audit.service_subject, subject);
    assert_eq!(decision.audit.service_client_id, node.login);
    let mut forged = request.clone();
    forged.subject = Box::new(
        serde_json::from_value(json!({"type":"user","id":"01234567-89ab-4cde-8fab-0123456789ab"}))
            .expect("different actor"),
    );
    assert!(
        matches!(policy::evaluate_permission(&config, policy::EvaluatePermissionParams { permission_evaluation_request:forged }).await,
        Err(of_management_client::apis::Error::ResponseError(r)) if r.status.as_u16() == 403),
        "User cannot assert another actor"
    );
    let mut cross_application = request.clone();
    cross_application.action = of_management_client::models::IdentityAction::ClientsRead;
    cross_application.target = Box::new(serde_json::from_value(json!({"scope":"application", "applicationId":"01234567-89ab-4cde-8fab-0123456789ab", "resource":{"type":"client","clientId":node.login}})).expect("different valid Application target"));
    assert!(
        matches!(policy::evaluate_permission(&config, policy::EvaluatePermissionParams { permission_evaluation_request:cross_application }).await,
        Err(of_management_client::apis::Error::ResponseError(r)) if r.status.as_u16() == 403),
        "User cannot assert another Application scope"
    );
    let wrong_audience = of_management_client::apis::configuration::Configuration {
        bearer_access_token: Some(session.clone()),
        ..config.clone()
    };
    assert!(
        matches!(policy::evaluate_permission(&wrong_audience, policy::EvaluatePermissionParams { permission_evaluation_request:request.clone() }).await,
        Err(of_management_client::apis::Error::ResponseError(r)) if matches!(r.status.as_u16(), 401 | 403)),
        "IdP session is not Management authority"
    );
    let mut ungranted = request.clone();
    ungranted.action = of_management_client::models::IdentityAction::DeviceRestrictionsRead;
    let denied = policy::evaluate_permission(
        &config,
        policy::EvaluatePermissionParams {
            permission_evaluation_request: ungranted.clone(),
        },
    )
    .await
    .unwrap_or_else(|_| panic!("negative decision is not an outage"));
    assert!(!denied.allowed);
    assert_eq!(*denied.request, ungranted);
    assert_eq!(*denied.audit.actor, *request.subject);
    assert!(
        matches!(of_device_client::apis::default_api::get_device_restriction(&device,
        of_device_client::apis::default_api::GetDeviceRestrictionParams { device_id:local.id }).await,
        Err(of_device_client::apis::Error::ResponseError(r)) if r.status.as_u16() == 403),
        "ordinary owner enforces the ungranted action"
    );
    of_device_client::apis::default_api::list_devices(&device)
        .await
        .unwrap_or_else(|_| panic!("distinct granted action remains allowed"));
    let read_only = idp::token(&node.config(&node.idp), idp::TokenParams {
        grant_type:Some("urn:ietf:params:oauth:grant-type:token-exchange".into()), client_id:Some(node.login.clone()), subject_token:Some(session.clone()),
        subject_token_type:Some(models::SubjectTokenType::UrnColonIetfColonParamsColonOauthColonTokenTypeColonAccessToken), resource:Some("storage:filesystems".into()),
        authorization_details:Some(r#"[{"type":"storage","actions":["read"]}]"#.into()), ..Default::default()
    }).await.unwrap_or_else(|_| panic!("ordinary read-only resource token")).access_token;
    let storage_config = of_storage_client::apis::configuration::Configuration {
        client: node.http.clone(),
        base_path: node.storage.clone(),
        bearer_access_token: Some(read_only),
        ..Default::default()
    };
    let before = storage::get_file_system(
        &storage_config,
        storage::GetFileSystemParams {
            filesystem_id: resource.clone(),
        },
    )
    .await
    .unwrap_or_else(|_| panic!("read-only authorized read"));
    assert!(
        matches!(storage::delete_file_system(&storage_config, storage::DeleteFileSystemParams { filesystem_id:resource.clone() }).await,
        Err(of_storage_client::apis::Error::ResponseError(r)) if r.status.as_u16() == 403),
        "read permission cannot delete resource"
    );
    let wrong_audience = of_storage_client::apis::configuration::Configuration {
        bearer_access_token: Some(session.clone()),
        ..storage_config.clone()
    };
    assert!(
        matches!(storage::get_file_system(&wrong_audience, storage::GetFileSystemParams { filesystem_id:resource.clone() }).await,
        Err(of_storage_client::apis::Error::ResponseError(r)) if matches!(r.status.as_u16(), 401 | 403)),
        "IdP audience is not resource authority"
    );
    let database_token = idp::token(&node.config(&node.idp), idp::TokenParams {
        grant_type:Some("urn:ietf:params:oauth:grant-type:token-exchange".into()), client_id:Some(node.login.clone()), subject_token:Some(session.clone()),
        subject_token_type:Some(models::SubjectTokenType::UrnColonIetfColonParamsColonOauthColonTokenTypeColonAccessToken), resource:Some("storage:databases".into()),
        authorization_details:Some(r#"[{"type":"storage","actions":["read"]}]"#.into()), ..Default::default()
    }).await.unwrap_or_else(|_| panic!("ordinary database-kind token")).access_token;
    let wrong_kind = of_storage_client::apis::configuration::Configuration {
        bearer_access_token: Some(database_token),
        ..storage_config.clone()
    };
    assert!(
        matches!(storage::get_file_system(&wrong_kind, storage::GetFileSystemParams { filesystem_id:resource.clone() }).await,
        Err(of_storage_client::apis::Error::ResponseError(r)) if r.status.as_u16() == 401),
        "database authority cannot read filesystem resources"
    );
    let after = storage::get_file_system(
        &storage_config,
        storage::GetFileSystemParams {
            filesystem_id: resource,
        },
    )
    .await
    .unwrap_or_else(|_| panic!("denied mutations preserve resource"));
    assert_eq!(before, after);
    retained_socket_denial(&node, &user, &session, &before.id).await;
}

async fn retained_socket_denial(node: &Node, user: &Configuration, session: &str, resource: &str) {
    use futures_util::{SinkExt, StreamExt};
    use rustls::pki_types::{CertificateDer, pem::PemObject};
    use storage_model::{StorageRequest, StorageResponse};
    use tokio_tungstenite::{Connector, connect_async_tls_with_config, tungstenite::Message};

    let token = node.exchange(session, "storage:filesystems", "").await;
    let mut url = reqwest::Url::parse(&node.storage).expect("explicit Storage base");
    url.set_scheme(if node.storage.starts_with("https:") {
        "wss"
    } else {
        "ws"
    })
    .expect("resource WebSocket scheme");
    url.query_pairs_mut()
        .append_pair("access_token", &token)
        .append_pair("filesystem_id", resource);
    let connector = if node.storage.starts_with("https:") {
        let pem = fs::read(node.root.path().join("roots.pem")).expect("isolated native roots");
        let mut roots = rustls::RootCertStore::empty();
        for certificate in CertificateDer::pem_slice_iter(&pem) {
            roots
                .add(certificate.expect("fixture CA certificate"))
                .expect("trusted fixture CA");
        }
        Some(Connector::Rustls(std::sync::Arc::new(
            rustls::ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth(),
        )))
    } else {
        None
    };
    let (mut socket, _) = tokio::time::timeout(
        Duration::from_secs(30),
        connect_async_tls_with_config(url.as_str(), None, false, connector.clone()),
    )
    .await
    .expect("bounded native resource connection")
    .unwrap_or_else(|_| panic!("authorized resource connection"));
    let write = StorageRequest::Write {
        path: "retained.txt".into(),
        content: b"authorized before denial".to_vec(),
    };
    socket
        .send(Message::Text(
            serde_json::to_string(&write)
                .expect("native request")
                .into(),
        ))
        .await
        .expect("authorized native write");
    let frame = tokio::time::timeout(Duration::from_secs(30), socket.next())
        .await
        .expect("bounded native write response")
        .expect("write response")
        .expect("valid frame");
    assert!(matches!(
        serde_json::from_str::<StorageResponse>(frame.to_text().expect("text response"))
            .expect("typed write response"),
        StorageResponse::Written { .. }
    ));
    socket
        .send(Message::Text(
            serde_json::to_string(&StorageRequest::Read {
                path: "retained.txt".into(),
            })
            .expect("native read")
            .into(),
        ))
        .await
        .expect("native read");
    let frame = tokio::time::timeout(Duration::from_secs(30), socket.next())
        .await
        .expect("bounded native read response")
        .expect("read response")
        .expect("valid frame");
    assert_eq!(
        serde_json::from_str::<StorageResponse>(frame.to_text().expect("text response"))
            .expect("typed read response"),
        StorageResponse::Read {
            content: b"authorized before denial".to_vec()
        }
    );
    let subject = idp::userinfo(user)
        .await
        .unwrap_or_else(|_| panic!("User before deletion"))
        .sub;
    let mut administrator = node.config(&node.idp);
    administrator.bearer_access_token = Some(node.exchange(session, &node.idp, "").await);
    idp::delete_user(&administrator, idp::DeleteUserParams { user_id: subject })
        .await
        .unwrap_or_else(|_| panic!("ordinary authorized User deletion"));
    socket
        .send(Message::Text(
            serde_json::to_string(&write)
                .expect("native denied request")
                .into(),
        ))
        .await
        .expect("subsequent retained-socket operation");
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            match socket.next().await {
                Some(Ok(Message::Text(body))) => {
                    assert!(
                        matches!(
                            serde_json::from_str::<StorageResponse>(&body).expect("typed denial"),
                            StorageResponse::Error { .. }
                        ),
                        "existing connection must not retain User authority"
                    );
                    break;
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
                _ => panic!("unexpected denied resource frame"),
            }
        }
    })
    .await
    .expect("bounded subsequent-operation denial");
    assert!(
        matches!(tokio::time::timeout(Duration::from_secs(30), connect_async_tls_with_config(url.as_str(), None, false, connector)).await.expect("bounded reconnect"),
        Err(tokio_tungstenite::tungstenite::Error::Http(response)) if response.status().as_u16() == 401),
        "deleted User cannot reconnect"
    );
    let storage_config = of_storage_client::apis::configuration::Configuration {
        client: node.http.clone(),
        base_path: node.storage.clone(),
        bearer_access_token: Some(token),
        ..Default::default()
    };
    assert!(
        matches!(storage::get_file_system(&storage_config, storage::GetFileSystemParams { filesystem_id:resource.into() }).await,
        Err(of_storage_client::apis::Error::ResponseError(r)) if r.status.as_u16() == 401),
        "deleted User cannot use catalog"
    );
    of_device_client::apis::default_api::health(&device_config(node))
        .await
        .expect("denial is not process loss");
}
