use std::time::{Duration, Instant};

use of_idp_client::{
    apis::{
        configuration::Configuration, default_api as idp, independent_setup_api as independent,
        setup_api as setup,
    },
    models,
};

use super::{Node, authorization_denied, device_config};

pub(super) async fn refresh_grant(
    node: &Node,
    client_id: &str,
    grant: &str,
) -> Result<models::TokenResponse, of_idp_client::apis::Error<idp::TokenError>> {
    idp::token(
        &node.config(&node.idp),
        idp::TokenParams {
            grant_type: Some("refresh_token".into()),
            client_id: Some(client_id.into()),
            refresh_token: Some(grant.into()),
            ..Default::default()
        },
    )
    .await
}

pub(super) fn denied_refresh(
    result: Result<models::TokenResponse, of_idp_client::apis::Error<idp::TokenError>>,
) {
    match result {
        Err(of_idp_client::apis::Error::ResponseError(response)) => {
            assert!(
                response.status.is_client_error(),
                "denial is not a server outage"
            );
            let body: serde_json::Value =
                serde_json::from_str(&response.content).expect("OAuth denial body");
            assert_eq!(body["error"], "invalid_grant");
        }
        _ => panic!("consumed or revoked refresh grant must be denied"),
    }
}

pub(super) async fn prepare_learned_grants(source: &Node) -> [String; 3] {
    let mut grants = Vec::new();
    for _ in 0..3 {
        grants.push(
            source
                .login_response()
                .await
                .refresh_token
                .flatten()
                .expect("ordinary refresh grant"),
        );
    }
    assert!(
        grants[0] != grants[1],
        "consumption and revocation grants are distinct"
    );
    assert!(
        grants[1] != grants[2],
        "revocation and positive-control grants are distinct"
    );
    refresh_grant(source, &source.login, &grants[0])
        .await
        .unwrap_or_else(|_| panic!("consume source refresh grant"));
    idp::revoke(
        &source.config(&source.idp),
        idp::RevokeParams {
            token: grants[1].clone(),
            token_type_hint: Some("refresh_token".into()),
        },
    )
    .await
    .unwrap_or_else(|_| panic!("ordinary source refresh revocation"));
    let grants: [String; 3] = grants
        .try_into()
        .unwrap_or_else(|_| panic!("three distinct grant cases"));
    assert_learned_grants(source, &source.login, &grants).await;
    grants
}

pub(super) async fn assert_learned_grants(node: &Node, client_id: &str, grants: &[String; 3]) {
    for grant in &grants[..2] {
        denied_refresh(refresh_grant(node, client_id, grant).await);
    }
}

pub(super) enum JoinAdmission {
    Unified(Configuration, models::JoinRequest),
    Independent(Vec<Configuration>, models::IndependentCreateRequest),
}

impl JoinAdmission {
    pub(super) async fn submit(&self) {
        match self {
            Self::Unified(config, request) => {
                setup::setup_join(
                    config,
                    setup::SetupJoinParams {
                        join_request: request.clone(),
                    },
                )
                .await
                .unwrap_or_else(|_| panic!("exact Unified Join admission"));
            }
            Self::Independent(configs, request) => {
                for config in configs {
                    independent::independent_setup_join(
                        config,
                        independent::IndependentSetupJoinParams {
                            independent_create_request: request.clone(),
                        },
                    )
                    .await
                    .unwrap_or_else(|_| panic!("exact independent worker Join admission"));
                }
            }
        }
    }

    async fn intents(&self) -> Vec<serde_json::Value> {
        match self {
            Self::Unified(config, _) => vec![
                serde_json::to_value(
                    setup::setup_intent(config)
                        .await
                        .unwrap_or_else(|_| panic!("protected retained Join intent")),
                )
                .expect("public intent"),
            ],
            Self::Independent(configs, _) => {
                let mut intents = Vec::new();
                for config in configs {
                    intents.push(
                        serde_json::to_value(
                            independent::independent_setup_recover(config)
                                .await
                                .unwrap_or_else(|_| panic!("protected owner Join intent")),
                        )
                        .expect("public owner intent"),
                    );
                }
                intents
            }
        }
    }

    async fn cancel(&self) {
        match self {
            Self::Unified(config, _) => {
                let intent = setup::setup_intent(config)
                    .await
                    .unwrap_or_else(|_| panic!("retained cancellation intent"));
                let anonymous = Configuration {
                    api_key: None,
                    ..config.clone()
                };
                assert!(
                    matches!(setup::setup_cancel(&anonymous, setup::SetupCancelParams { setup_intent:intent.clone() }).await,
                    Err(of_idp_client::apis::Error::ResponseError(r)) if r.status.as_u16() == 401),
                    "cancellation needs the host capability"
                );
                setup::setup_cancel(
                    config,
                    setup::SetupCancelParams {
                        setup_intent: intent,
                    },
                )
                .await
                .unwrap_or_else(|_| panic!("signal Unified cancellation"));
            }
            Self::Independent(configs, _) => {
                for config in configs {
                    let status = independent::independent_setup_status(config)
                        .await
                        .expect("worker before cancellation");
                    assert!(!status.ready);
                    if status.state == "unfinished" {
                        continue;
                    }
                    let intent = independent::independent_setup_recover(config)
                        .await
                        .unwrap_or_else(|_| panic!("exact owner cancellation intent"));
                    let anonymous = Configuration {
                        api_key: None,
                        ..config.clone()
                    };
                    assert!(
                        matches!(independent::independent_setup_cancel(&anonymous, independent::IndependentSetupCancelParams { independent_create_intent:intent.clone() }).await,
                        Err(of_idp_client::apis::Error::ResponseError(r)) if r.status.as_u16() == 401),
                        "cancellation needs this worker capability"
                    );
                    independent::independent_setup_cancel(
                        config,
                        independent::IndependentSetupCancelParams {
                            independent_create_intent: intent,
                        },
                    )
                    .await
                    .unwrap_or_else(|_| panic!("signal independent worker cancellation"));
                }
            }
        }
    }

    async fn interrupt_and_reopen(&self, target: &mut Node) {
        let before = self.intents().await;
        self.cancel().await;
        self.unfinished(target).await;
        while let Some(process) = target.processes.pop() {
            drop(process);
        }
        for (name, command) in &mut target.commands {
            assert!(
                name != "management" && name != "identity-database",
                "pending local-owner Node has no ready consumers"
            );
            target
                .processes
                .push(Node::spawn(name, command, target.root.path()));
        }
        self.unfinished(target).await;
        assert_eq!(
            self.intents().await,
            before,
            "cancellation and process interruption retain the exact operation, not rollback"
        );
    }

    async fn unfinished(&self, target: &mut Node) {
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            target.alive();
            let states = match self {
                Self::Unified(config, _) => setup::setup_status(config)
                    .await
                    .ok()
                    .map(|s| vec![(s.ready, s.state, s.stage)]),
                Self::Independent(configs, _) => {
                    let mut states = Vec::new();
                    for config in configs {
                        if let Ok(s) = independent::independent_setup_status(config).await {
                            states.push((s.ready, s.state, s.stage));
                        }
                    }
                    (states.len() == configs.len()).then_some(states)
                }
            };
            if let Some(states) = &states {
                assert!(
                    states.iter().all(|(ready, _, _)| !ready),
                    "rejected membership must never become ready"
                );
                if states.iter().all(|(_, state, _)| state == "unfinished") {
                    break;
                }
            }
            assert!(
                Instant::now() < deadline,
                "unfinished Join timeout; last states: {states:?}"
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

pub(super) async fn pending_offer(
    source: &mut Node,
    target: &mut Node,
    approval: &of_device_client::apis::configuration::Configuration,
    independent_layout: bool,
) -> serde_json::Value {
    use of_device_client::apis::{
        independent_join_api as independent_joins, node_join_api as joins,
    };
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        source.alive();
        target.alive();
        let offers = if independent_layout {
            independent_joins::independent_join_pending(approval)
                .await
                .unwrap_or_else(|_| panic!("authorized independent pending list"))
        } else {
            joins::node_join_pending(approval)
                .await
                .unwrap_or_else(|_| panic!("authorized pending list"))
                .as_array()
                .expect("pending list")
                .clone()
        };
        if let Some(offer) = offers.into_iter().find(|offer| {
            offer["request"]["name"] == "Joining System Node"
                || offer["request"]["operation"]["node"] == "Joining System Node"
        }) {
            return offer;
        }
        assert!(Instant::now() < deadline, "pending authorized Join timeout");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

pub(super) async fn retry_rejected_join(
    source: &mut Node,
    target: &mut Node,
    approval: &of_device_client::apis::configuration::Configuration,
    independent_layout: bool,
    admission: &JoinAdmission,
    first: serde_json::Value,
) -> serde_json::Value {
    use of_device_client::apis::{
        independent_join_api as independent_joins, node_join_api as joins,
    };
    admission.submit().await;
    let pending = pending_offer(source, target, approval, independent_layout).await;
    assert_eq!(
        pending["request"], first["request"],
        "active exact retry retains the request"
    );
    admission.interrupt_and_reopen(target).await;
    assert_offer_released(
        source,
        approval,
        independent_layout,
        first["offer_id"].as_str().expect("interrupted offer"),
    )
    .await;
    admission.submit().await;
    let same = pending_offer(source, target, approval, independent_layout).await;
    assert_eq!(
        same["request"], first["request"],
        "pending exact retry retains signer and endpoint"
    );
    let id = same["offer_id"]
        .as_str()
        .expect("exact rejected offer")
        .to_owned();
    let anonymous = device_config(source);
    if independent_layout {
        authorization_denied(
            independent_joins::independent_join_reject(
                &anonymous,
                independent_joins::IndependentJoinRejectParams { id: id.clone() },
            )
            .await,
        );
        independent_joins::independent_join_reject(
            approval,
            independent_joins::IndependentJoinRejectParams { id: id.clone() },
        )
        .await
        .unwrap_or_else(|_| panic!("authorized independent rejection"));
    } else {
        authorization_denied(
            joins::node_join_reject(&anonymous, joins::NodeJoinRejectParams { id: id.clone() })
                .await,
        );
        joins::node_join_reject(approval, joins::NodeJoinRejectParams { id: id.clone() })
            .await
            .unwrap_or_else(|_| panic!("authorized Unified rejection"));
    }
    if independent_layout {
        admission.interrupt_and_reopen(target).await;
    } else {
        admission.unfinished(target).await;
    }
    if independent_layout {
        assert!(
            matches!(independent_joins::independent_join_approve(approval, independent_joins::IndependentJoinApproveParams { id }).await,
            Err(of_device_client::apis::Error::ResponseError(r)) if r.status.as_u16() == 409),
            "rejected offer is not approvable"
        );
    } else {
        assert!(
            matches!(joins::node_join_approve(approval, joins::NodeJoinApproveParams { id }).await,
            Err(of_device_client::apis::Error::ResponseError(r)) if r.status.as_u16() == 404),
            "rejected offer is not approvable"
        );
    }
    admission.submit().await;
    let retried = pending_offer(source, target, approval, independent_layout).await;
    assert_eq!(
        retried["request"], first["request"],
        "rejection retains exact Node, signer and endpoint"
    );
    retried
}

async fn assert_offer_released(
    source: &mut Node,
    approval: &of_device_client::apis::configuration::Configuration,
    independent_layout: bool,
    id: &str,
) {
    use of_device_client::apis::{
        independent_join_api as independent_joins, node_join_api as joins,
    };
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        source.alive();
        let offers = if independent_layout {
            independent_joins::independent_join_pending(approval)
                .await
                .unwrap_or_else(|_| panic!("pending independent release evidence"))
        } else {
            joins::node_join_pending(approval)
                .await
                .unwrap_or_else(|_| panic!("pending Unified release evidence"))
                .as_array()
                .expect("pending list")
                .clone()
        };
        if offers.iter().all(|offer| offer["offer_id"] != id) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "interrupted source offer release timeout"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    if independent_layout {
        assert!(
            matches!(independent_joins::independent_join_approve(approval, independent_joins::IndependentJoinApproveParams { id:id.into() }).await,
            Err(of_device_client::apis::Error::ResponseError(r)) if r.status.as_u16() == 409),
            "interrupted offer cannot be approved"
        );
    } else {
        assert!(
            matches!(joins::node_join_approve(approval, joins::NodeJoinApproveParams { id:id.into() }).await,
            Err(of_device_client::apis::Error::ResponseError(r)) if r.status.as_u16() == 404),
            "interrupted offer cannot be approved"
        );
    }
}
