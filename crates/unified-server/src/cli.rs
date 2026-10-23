use std::{
    io::{self, IsTerminal},
    path::PathBuf,
};

use clap::{Parser, Subcommand};
use cli::shutdown_signal;
use dialoguer::{Input, Password};
use env_logger::Env;
use idp_server::{
    BootstrapConfig, BootstrapPurpose, CredentialBootstrap, LocalJoin, LocalProvisioner,
    bootstrap_config::present,
};

use crate::{UnifiedBootstrapHost, UnifiedConfig, discover_installation_issuer};

#[derive(Parser, Debug)]
#[command(version, about = "Create, join and serve an independently usable Node")]
struct UnifiedArgs {
    /// Optional explicit composition; ordinary bootstrap needs no config file.
    #[arg(long, short = 'c', global = true)]
    config: Option<PathBuf>,
    /// Optional alternative to the per-user default Node directory.
    #[arg(long, global = true, conflicts_with = "config")]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<UnifiedCommand>,
}
#[derive(Subcommand, Debug)]
enum UnifiedCommand {
    Serve {
        #[arg(long)]
        host: Option<std::net::IpAddr>,
        #[arg(long, short = 'p')]
        port: Option<u16>,
    },
    /// Create a complete Node with derived settings, or continue its exact retained intent.
    Create {
        #[arg(long)]
        administrator: Option<String>,
        #[arg(long)]
        node: Option<String>,
        /// Optional automation: operator-only file, never a password argument.
        #[arg(long)]
        password_file: Option<PathBuf>,
    },
    /// Join a trusted Node; prompt for the pinned endpoint if not supplied.
    Join {
        #[arg(long)]
        peer: Option<String>,
        #[arg(long)]
        node: Option<String>,
    },
    #[cfg(feature = "completions")]
    Completions { shell: clap_complete::Shell },
}

pub async fn run() -> io::Result<()> {
    let args = UnifiedArgs::parse();
    #[cfg(feature = "completions")]
    if let Some(UnifiedCommand::Completions { shell }) = &args.command {
        use clap::CommandFactory;
        clap_complete::generate(
            *shell,
            &mut UnifiedArgs::command(),
            "unified-server",
            &mut io::stdout(),
        );
        return Ok(());
    }
    idp_server::local_provisioning::require_unix_bootstrap()?;
    let setup = matches!(
        args.command,
        Some(UnifiedCommand::Create { .. } | UnifiedCommand::Join { .. })
    );
    let port = std::env::var("PORT")
        .ok()
        .map(|value| parse_port(&value))
        .transpose()?;
    let public_base = std::env::var("PORTLESS_URL").ok();
    let mut listen = std::net::SocketAddr::from(([127, 0, 0, 1], port.unwrap_or(3000)));
    if let Some(UnifiedCommand::Serve { host, port }) = &args.command {
        if let Some(host) = host {
            listen.set_ip(*host);
        }
        if let Some(port) = port {
            listen.set_port(*port);
        }
    }
    let base = public_base
        .clone()
        .unwrap_or_else(|| format!("http://{listen}"));
    let mut selected_peer = None;
    let mut fresh_host = false;
    let mut host_guard = if args.config.is_none() {
        let root = match &args.data_dir {
            Some(root) => root.clone(),
            None => BootstrapConfig::<UnifiedBootstrapHost>::default_directory()?,
        };
        let host = if present(&root)? {
            BootstrapConfig::<UnifiedBootstrapHost>::open(&root)?
        } else {
            fresh_host = true;
            match &args.command {
                Some(UnifiedCommand::Create { .. }) => {
                    BootstrapConfig::<UnifiedBootstrapHost>::create_http(
                        &root,
                        BootstrapPurpose::Create,
                        None,
                        listen,
                        base.clone(),
                    )?
                }
                Some(UnifiedCommand::Join { peer, .. }) => {
                    let peer = peer_input(peer.as_deref())?;
                    println!(
                        "Learning Installation identity from the authenticated trusted peer; its authorized join interface must be open."
                    );
                    let issuer = discover_installation_issuer(&peer).await?;
                    let host = BootstrapConfig::<UnifiedBootstrapHost>::create_http(
                        &root,
                        BootstrapPurpose::Join,
                        Some((&peer, &issuer)),
                        listen,
                        base.clone(),
                    )?;
                    selected_peer = Some(peer);
                    host
                }
                _ => BootstrapConfig::<UnifiedBootstrapHost>::create_http(
                    &root,
                    BootstrapPurpose::AwaitingChoice,
                    None,
                    listen,
                    base.clone(),
                )?,
            }
        };
        let expected = match &args.command {
            Some(UnifiedCommand::Create { .. }) => Some(BootstrapPurpose::Create),
            Some(UnifiedCommand::Join { .. }) => Some(BootstrapPurpose::Join),
            _ => None,
        };
        if expected.is_some_and(|purpose| purpose != host.purpose()) {
            return Err(invalid(
                "command conflicts with retained Node bootstrap purpose",
            ));
        }
        if let Some(UnifiedCommand::Join {
            peer: Some(peer), ..
        }) = &args.command
            && host.trusted_peer() != Some(peer.as_str())
        {
            return Err(invalid("peer differs from retained Node intent"));
        }
        if selected_peer.is_none() {
            selected_peer = host.trusted_peer().map(str::to_owned);
        }
        Some(host)
    } else {
        None
    };
    let mut config = if let Some(host) = &host_guard {
        host.config()
    } else {
        UnifiedConfig::try_from(
            args.config
                .as_ref()
                .expect("explicit config path")
                .as_path(),
        )
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?
    };
    if let Some(port) = port {
        config.listen_addr.set_port(port);
    } else if config.listen_addr.port() == 0 {
        config.listen_addr.set_port(3000);
    }

    if let Some(base) = public_base {
        crate::setup_host::allowed_origin(&base)?;
        if args.config.is_some() {
            let base = base.trim_end_matches('/');
            config.idp.api_public_uri = format!("{base}/idp");
            config.idp.service_audience = Some(format!("{base}/idp"));
            config.management.api_public_uri = format!("{base}/management");
            config.device.api_public_uri = format!("{base}/device");
            config.device.storage_audience = format!("{base}/storage");
            config.storage.api_public_uri = format!("{base}/storage");
        }
    }
    env_logger::Builder::from_env(
        Env::default().default_filter_or(format!("{},automerge=warn", config.idp.log_level)),
    )
    .init();
    if let Some(UnifiedCommand::Serve { host, port }) = &args.command {
        if let Some(host) = host {
            config.listen_addr.set_ip(*host);
        }
        if let Some(port) = port {
            config.listen_addr.set_port(*port);
        }
    }
    crate::LocalTls::api_base(config.listen_addr)?;
    if let Some(host) = &mut host_guard {
        host.set_runtime_config(config.clone());
    }
    #[cfg(unix)]
    {
        if args.config.is_some() {
            (host_guard, fresh_host) = explicit_startup_host(&config, setup)?;
        }
        if host_guard.as_ref().is_some_and(|host| {
            host.purpose() == BootstrapPurpose::AwaitingChoice || host.setup_attempted()
        }) {
            let host = host_guard.take().expect("neutral host guard");
            let setup_host = if fresh_host {
                crate::SetupHost::create_http(host)?
            } else {
                crate::SetupHost::open_http(host)?
            };
            let cancellation = tokio_util::sync::CancellationToken::new();
            let shutdown = shutdown_signal(cancellation.clone());
            let serve = setup_host.serve(cancellation.clone());
            tokio::pin!(serve);
            tokio::select! {
                result = &mut serve => return result,
                () = shutdown => return serve.await,
            }
        }
    }
    let retained_setup = present(&config.local_provisioning_directory())?;
    if !retained_setup
        && let Some(UnifiedCommand::Join { peer, .. }) = &args.command
        && selected_peer.is_none()
    {
        selected_peer = Some(peer_input(peer.as_deref())?);
    }
    let automation_password = if let Some(UnifiedCommand::Create {
        password_file: Some(path),
        ..
    }) = &args.command
    {
        Some(idp_server::local_provisioning::read_password(path)?)
    } else {
        None
    };
    if !retained_setup
        && matches!(
            &args.command,
            Some(UnifiedCommand::Create {
                password_file: None,
                ..
            })
        )
    {
        interactive_terminal()?;
    }
    let credential_guard = if present(&config.credential_bootstrap_directory())? {
        CredentialBootstrap::<UnifiedBootstrapHost>::open(config.clone())?
    } else if setup {
        CredentialBootstrap::<UnifiedBootstrapHost>::create(config.clone())?
    } else {
        return Err(invalid(
            "no retained host bootstrap; explicitly create or join first",
        ));
    };

    config = credential_guard.load()?;
    match args.command {
        Some(UnifiedCommand::Create {
            administrator,
            node,
            ..
        }) => {
            if retained_setup && (administrator.is_some() || node.is_some()) {
                return Err(invalid(
                    "continuation uses retained names; omit name overrides",
                ));
            }
            let mut local = if retained_setup {
                LocalProvisioner::<UnifiedBootstrapHost>::open(config)?
            } else {
                let password = match automation_password {
                    Some(password) => password,
                    None => password_input(true)?,
                };
                let mut local = LocalProvisioner::<UnifiedBootstrapHost>::create_with_password(
                    config,
                    administrator.as_deref().unwrap_or("operator"),
                    node.as_deref().unwrap_or("Local Node"),
                    &password,
                )?;
                local.provision_http(&password).await?;
                print_login(&local.login_client()?);
                println!(
                    "Node created with retained settings, credentials and verified HTTP. Start with serve."
                );
                return Ok(());
            };
            if local.is_complete()? {
                local.verify_http().await?;
            } else {
                let password = match automation_password {
                    Some(password) => password,
                    None => password_input(false)?,
                };
                local.provision_http(&password).await?;
            }
            print_login(&local.login_client()?);
            println!("Node create is complete and production HTTP is qualified. Start with serve.");
            return Ok(());
        }
        Some(UnifiedCommand::Join { peer, node }) => {
            let mut local = if retained_setup {
                if node.is_some() {
                    return Err(invalid("continuation uses the retained Node name"));
                }
                LocalJoin::<UnifiedBootstrapHost>::open(config)?
            } else {
                let peer = selected_peer
                    .or(peer)
                    .ok_or_else(|| invalid("trusted peer evidence is required"))?;
                LocalJoin::<UnifiedBootstrapHost>::create(
                    config,
                    &peer,
                    node.as_deref().unwrap_or("Local Node"),
                )?
            };
            println!("Waiting for authorized Node approval and public context bootstrap.");
            local.join_http().await?;
            print_login(&local.login_client());
            println!(
                "Node joined with retained settings, credentials and verified HTTP. Start with serve."
            );
            return Ok(());
        }
        _ => (),
    }
    let public_idp = config.idp.api_public_uri.clone();
    let issuer = config.idp.oauth2.issuer.clone();
    let idp_audience = config.idp_audience().to_owned();
    let management_audience = config.device.storage_audience.clone();
    let (runtime, create_guard, join_guard) =
        if present(&config.local_provisioning_directory().join("join.json"))? {
            let local = LocalJoin::<UnifiedBootstrapHost>::open(config)?;
            (local.build_runtime().await?, None, Some(local))
        } else {
            let (runtime, local) =
                LocalProvisioner::<UnifiedBootstrapHost>::open_runtime(config).await?;
            (runtime, local, None)
        };
    println!("Node endpoint: {}", runtime.endpoint_id());
    let base = format!("http://{}", runtime.local_addr());
    println!("Local HTTP: {base}");
    println!("Public IdP: {public_idp}");
    println!("Installation issuer: {issuer}");
    println!("IdP token endpoint: {base}/idp/oauth2/token");
    println!("Public APIs: {base}/idp {base}/management {base}/device {base}/storage");
    println!("IdP audience: {idp_audience}");
    println!(
        "Management audience: {} (Storage: {management_audience})",
        management_service::MANAGEMENT_APPLICATION_URI
    );
    if let Some(local) = &create_guard {
        print_login(&local.login_client()?);
    }
    if let Some(local) = &join_guard {
        print_login(&local.login_client());
    }
    let cancellation = runtime.cancellation_token();
    let mut server_task = tokio::spawn(runtime.serve());
    tokio::select! {
        result = &mut server_task => result.map_err(io::Error::other)??,
        () = shutdown_signal(cancellation) => { server_task.await.map_err(io::Error::other)??; }
    }
    drop((create_guard, join_guard, credential_guard, host_guard));
    Ok(())
}

#[cfg(unix)]
fn explicit_startup_host(
    config: &UnifiedConfig,
    setup: bool,
) -> io::Result<(Option<BootstrapConfig<UnifiedBootstrapHost>>, bool)> {
    let root = BootstrapConfig::<UnifiedBootstrapHost>::explicit_directory(config);
    if present(&root)? {
        if setup {
            return Err(invalid(
                "retained neutral host requires code-authorized purpose selection; not implemented in this slice",
            ));
        }
        Ok((
            Some(BootstrapConfig::<UnifiedBootstrapHost>::open_explicit(
                config.clone(),
            )?),
            false,
        ))
    } else if !setup && !present(&config.credential_bootstrap_directory())? {
        Ok((
            Some(BootstrapConfig::<UnifiedBootstrapHost>::create_explicit_http(config.clone())?),
            true,
        ))
    } else {
        Ok((None, false))
    }
}

fn print_login(login: &idp_model::contract::ClientRegistration) {
    println!(
        "Operator login Client: {}",
        login
            .client_id
            .as_deref()
            .expect("retained public login ID")
    );
    println!("Login scopes: {}", login.allowed_scopes.join(" "));
    println!(
        "Token-exchange audiences: {}",
        login.allowed_audiences.join(" ")
    );
}

fn interactive_terminal() -> io::Result<()> {
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        return Err(invalid(
            "interactive bootstrap requires a terminal; automate with --peer and/or protected --password-file",
        ));
    }
    Ok(())
}
fn password_input(confirm: bool) -> io::Result<String> {
    interactive_terminal()?;
    let mut prompt = Password::new().with_prompt("Initial administrator password (hidden)");
    if confirm {
        prompt = prompt.with_confirmation("Confirm password", "Passwords do not match");
    }
    let password = prompt.interact().map_err(io::Error::other)?;
    idp_server::local_provisioning::validate_password(&password)?;
    Ok(password)
}
fn peer_input(peer: Option<&str>) -> io::Result<String> {
    let peer = if let Some(peer) = peer {
        peer.to_owned()
    } else {
        interactive_terminal()?;
        Input::<String>::new()
            .with_prompt("Trusted peer endpoint ID (verify through a trusted channel)")
            .interact_text()
            .map_err(io::Error::other)?
    };
    device_service::hosted_join::NodeJoinRequest::validate_trusted_peer(&peer)?;
    Ok(peer)
}
fn parse_port(value: &str) -> io::Result<u16> {
    value
        .parse::<u16>()
        .ok()
        .filter(|port| *port != 0)
        .ok_or_else(|| invalid("PORT must be an integer from 1 to 65535"))
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::UnifiedArgs;
    use clap::Parser;

    #[test]
    fn default_logger_filter_quiets_automerge_and_preserves_configured_levels() {
        use log::{Level, Log, Metadata};

        let defaults = [
            idp_server::AppConfig::default().log_level,
            management_server::AppConfig::default().log_level,
            storage_server::AppConfig::default().log_level,
        ];
        for level in &defaults {
            assert_eq!(level, "info");
        }
        for level in defaults
            .iter()
            .map(String::as_str)
            .chain(["DEBUG", "trace"])
        {
            let logger = env_logger::Builder::new()
                .parse_filters(&format!("{level},automerge=warn"))
                .build();
            for (target, level, expected) in [
                ("unified_server", Level::Info, true),
                ("unified_server", Level::Debug, level != "info"),
                ("automerge", Level::Info, false),
                ("automerge::autocommit", Level::Debug, false),
                ("automerge::autocommit", Level::Warn, true),
            ] {
                assert_eq!(
                    logger.enabled(&Metadata::builder().target(target).level(level).build()),
                    expected,
                    "{target} {level}"
                );
            }
        }
    }
    #[tokio::test]
    async fn retained_host_accepts_only_configured_ui_origin_without_rewriting_identity() {
        use axum::{body::Body, http::Request};
        use tower::ServiceExt;

        let root = std::env::temp_dir().join(format!(
            "unified-proxy-origin-{}",
            idp_model::model::Id::now_v7()
        ));
        let host = crate::BootstrapConfig::create_http(
            &root,
            crate::BootstrapPurpose::AwaitingChoice,
            None,
            "127.0.0.1:3000".parse().expect("retained listener"),
            "http://127.0.0.1:3000".into(),
        )
        .expect("retained HTTP host");
        let original = host.config();
        drop(crate::SetupHost::create_http(host).expect("initial setup"));
        let metadata = std::fs::read(root.join("host.json")).expect("retained metadata");
        let mut host = crate::BootstrapConfig::open(&root).expect("reopen retained host");
        let mut config = host.config();
        config.idp.ui_public_uri = "https://of.localhost:1355".into();
        config.listen_addr.set_port(49152);
        assert_eq!(config.idp.api_public_uri, original.idp.api_public_uri);
        assert_eq!(
            config.management.api_public_uri,
            original.management.api_public_uri
        );
        assert_eq!(
            config.storage.api_public_uri,
            original.storage.api_public_uri
        );
        assert_eq!(config.idp_audience(), original.idp_audience());
        assert_eq!(
            config.device.storage_audience,
            original.management.storage_audience
        );
        assert_eq!(config.idp.oauth2.issuer, original.idp.oauth2.issuer);
        host.set_runtime_config(config);
        let setup =
            crate::SetupHost::open_http(host).expect("reopen through proxy without certificates");
        assert_eq!(setup.local_addr().port(), 49152);
        let cancellation = tokio_util::sync::CancellationToken::new();
        let worker = setup.into_worker(cancellation);
        let router = worker.router().expect("proxy origin router");
        for origin in [
            "https://of.localhost:1355",
            "https://unified.localhost:1355",
            "https://of.localhost:1356",
            "http://of.localhost:1355",
            "https://of.localhost.evil.invalid:1355",
            "https://evil.invalid",
        ] {
            let allowed = origin == "https://of.localhost:1355";
            for method in ["GET", "OPTIONS"] {
                let mut request = Request::builder()
                    .method(method)
                    .uri("/idp/setup/status")
                    .header("Origin", origin);
                if method == "OPTIONS" {
                    request = request.header("Access-Control-Request-Method", "GET");
                }
                let response = router
                    .clone()
                    .oneshot(request.body(Body::empty()).expect("status request"))
                    .await
                    .expect("status response");
                assert_eq!(
                    response.status(),
                    if allowed {
                        axum::http::StatusCode::OK
                    } else {
                        axum::http::StatusCode::FORBIDDEN
                    },
                    "{method} {origin}"
                );
                assert_eq!(
                    response.headers().get("Access-Control-Allow-Origin"),
                    allowed.then_some(&axum::http::HeaderValue::from_static(
                        "https://of.localhost:1355"
                    )),
                    "{method} {origin}"
                );
            }
        }
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/idp/setup/status")
                    .body(Body::empty())
                    .expect("status request without origin"),
            )
            .await
            .expect("status response without origin");
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        assert_eq!(
            std::fs::read(root.join("host.json")).expect("unchanged metadata"),
            metadata
        );
        assert!(!crate::LocalTls::directory(&original).exists());
        drop(worker);
        std::fs::remove_dir_all(root).expect("isolated cleanup");
    }

    #[test]
    fn port_environment_requires_a_nonzero_tcp_port() {
        assert_eq!(super::parse_port("3000").expect("default port"), 3000);
        assert_eq!(
            super::parse_port("49152").expect("proxy assigned port"),
            49152
        );
        for value in ["", "0", "65536", "invalid", "-1"] {
            assert!(super::parse_port(value).is_err(), "{value}");
        }
    }

    #[test]
    fn automatic_startup_requires_positively_absent_roots() {
        let root = std::env::temp_dir().join(format!(
            "unified-fresh-classification-{}",
            idp_model::model::Id::now_v7()
        ));
        let mut config = crate::config::test_config(&root);
        config.idp.ui_public_uri = "https://app.example".into();
        std::fs::create_dir_all(&config.storage.data_dir)
            .expect("retained storage with missing credentials");
        assert!(super::explicit_startup_host(&config, false).is_err());
        assert!(!crate::BootstrapConfig::explicit_directory(&config).exists());
        std::fs::remove_dir_all(&config.storage.data_dir).expect("remove only test retained root");
        let (host, fresh) =
            super::explicit_startup_host(&config, false).expect("genuinely absent roots");
        assert!(fresh);
        let host = host.expect("neutral host");
        assert!(host.config().idp.oauth2.issuer.is_empty());
        drop(host);
        std::fs::remove_dir_all(root).expect("isolated cleanup");
    }

    #[test]
    fn ordinary_bootstrap_has_no_required_config_names_or_password_file() {
        let ordinary = UnifiedArgs::try_parse_from(["unified-server"])
            .expect("ordinary startup without a subcommand");
        assert!(ordinary.command.is_none());
        assert!(ordinary.config.is_none());
        for command in ["create", "join", "serve"] {
            let args = UnifiedArgs::try_parse_from(["unified-server", command])
                .expect("ordinary no-config command");
            assert!(args.config.is_none());
            assert!(args.data_dir.is_none());
        }
        UnifiedArgs::try_parse_from([
            "unified-server",
            "create",
            "--password-file",
            "protected-password",
        ])
        .expect("optional automation");
        assert!(
            UnifiedArgs::try_parse_from(["unified-server", "create", "--password", "raw-password"])
                .is_err()
        );
        assert!(UnifiedArgs::try_parse_from(["unified-server", "provision"]).is_err());
        assert!(
            UnifiedArgs::try_parse_from(["unified-server", "join", "--skip-ca-install"]).is_err()
        );
    }
}
