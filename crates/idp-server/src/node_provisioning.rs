use std::{
    fs,
    io::{self, Write},
};

use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use db::{SqlTranslator, Value};
use idp_model::{
    contract::{
        ApplicationRegistration, ClientProfile, ClientRegistration, ClientType, EntityType,
        GrantType, IdpSignerRecord, JwkPublic, JwkPublicParameters, JwsAlgorithm, KeyUse,
        ReplicaMembership, TokenEndpointAuthMethod,
    },
    model::{Client, Id, Key},
};
use idp_service::{
    oauth2::NodeSignerBinding,
    replica::DbReplicaSignerRepo,
    repo::{ApplicationRepo, ClientRepo, KeyRepo, KeyService, PrivateKeyRepo, RepoError},
};
use iroh::EndpointId;
use key::{DerivationPath, DerivedKey, MasterKey};
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};

use crate::OwnerProvisioner;

#[derive(Deserialize, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
enum NodePreparation {
    NeverStarted,
    Attempted {
        binding: NodeSignerBinding,
        public_signer: Option<JwkPublic>,
    },
    Prepared {
        binding: NodeSignerBinding,
        public_signer: JwkPublic,
    },
}

impl<P: PrivateKeyRepo + Send + Sync> OwnerProvisioner<P> {
    /// Nonblocking, cross-handle/process exclusion on stable owner-controlled ancestry.
    /// The lock inode must remain in place until exclusive owner-data removal.
    fn setup_operation(&self) -> io::Result<fs::File> {
        let path = self.data_dir.join("setup-secret-operation.lock");
        match fs::symlink_metadata(&path) {
            Ok(metadata) if !metadata.file_type().is_file() => {
                return Err(invalid("setup operation lock must be a regular local file"));
            }
            Ok(_) => (),
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => return Err(error),
        }
        let mut options = fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(path)?;
        file.try_lock().map_err(|error| match error {
            fs::TryLockError::WouldBlock => io::Error::new(
                io::ErrorKind::WouldBlock,
                "another setup secret operation owns this directory; retry after it finishes",
            ),
            fs::TryLockError::Error(error) => error,
        })?;
        Ok(file)
    }

    pub async fn ensure_initial_user_root(&self, user: Id) -> io::Result<()> {
        let _operation = self.setup_operation()?;
        if user.is_nil() {
            return Err(invalid("initial User identity is required"));
        }
        self.oauth2
            .find_user_info(user)
            .await
            .map_err(|error| io::Error::other(format!("{error:?}")))?;
        let slot = format!("user:{user}");
        let inventory = self.ready_inventory()?;
        if inventory["owner"] != json!(user) {
            return Err(invalid("initial User conflicts with secret inventory"));
        }
        let reference = self.read_metadata_reference(&slot)?;
        if reference["state"] == "never-started" {
            if !self
                .oauth2
                .key_service
                .key_repo()
                .list_by_entity_type_and_id(EntityType::User, user)
                .await
                .map_err(io::Error::other)?
                .is_empty()
            {
                return Err(invalid(
                    "unexpected User root contradicts never-started capture",
                ));
            }
            let mut pending = reference.clone();
            pending["state"] = json!("pending");
            self.write_client_reference(&slot, &pending)?;
        } else if reference["state"] == "pending" {
            return Err(invalid(
                "unknown initial User history requires explicit recovery",
            ));
        }
        if reference["state"] == "never-started" {
            self.oauth2
                .key_service
                .create_initial_root_with_capture(
                    EntityType::User,
                    user,
                    "Local User".into(),
                    |key, paths| {
                        self.capture_initial_root(&slot, key, paths)
                            .map_err(|error| RepoError::InvalidInput(error.to_string()))
                    },
                )
                .await
                .map_err(io::Error::other)?;
        }
        self.verify_local_entity_root(EntityType::User, user, &slot)
            .await
    }

    pub async fn ensure_client_root(&self, client_id: &str) -> io::Result<()> {
        let _operation = self.setup_operation()?;
        self.ensure_client_root_locked(client_id).await
    }

    async fn ensure_client_root_locked(&self, client_id: &str) -> io::Result<()> {
        let client = self
            .oauth2
            .client_repo
            .find_client_by_client_id(client_id)
            .await
            .map_err(io::Error::other)?
            .ok_or_else(|| invalid("local setup client row is missing"))?;
        self.ready_inventory()?;
        let reference = self.read_metadata_reference(client_id)?;
        if reference["state"] != "resolved" || reference["entity_id"] != json!(client.id) {
            return Err(invalid("Client lacks owner-captured secret inventory"));
        }
        self.verify_local_entity_root(EntityType::Client, client.id, client_id)
            .await
    }

    async fn verify_local_entity_root(
        &self,
        entity: EntityType,
        id: Id,
        slot: &str,
    ) -> io::Result<()> {
        let keys = &self.oauth2.key_service;
        let reference = self.read_metadata_reference(slot)?;
        if reference["state"] != "resolved" || reference["entity_id"] != json!(id) {
            return Err(invalid(
                "entity conflicts with captured metadata-root inventory",
            ));
        }
        let root_id: Id = serde_json::from_value(reference["root_id"].clone())?;
        let initial = keys
            .key_repo()
            .find_by_id(root_id)
            .await
            .map_err(io::Error::other)?
            .ok_or_else(|| {
                invalid("captured root metadata is missing or inactive; explicit recovery required")
            })?;
        validate_metadata_root(&initial, entity, id)?;
        let paths = KeyService::<idp_service::replica::DbKeyRepo, P>::private_paths(&initial)
            .map_err(io::Error::other)?;
        if reference["paths"] != json!(paths.iter().map(ToString::to_string).collect::<Vec<_>>()) {
            return Err(invalid(
                "captured root metadata conflicts with retained identity",
            ));
        }
        let active = keys
            .key_repo()
            .find_active_entity_root_key(entity, id)
            .await
            .map_err(io::Error::other)?
            .ok_or_else(|| {
                invalid("active Principal root is missing; no replacement was created")
            })?;
        validate_metadata_root(&active, entity, id)?;
        Ok(())
    }

    pub async fn ensure_desktop_client(&self, client_id: Id) -> io::Result<ClientRegistration> {
        if client_id.is_nil() {
            return Err(invalid("stable desktop client ID is required"));
        }
        let client_id = client_id.to_string();
        let request = ClientRegistration {
            application: ApplicationRegistration {
                name: Some("Local Desktop".into()),
                uri: "local.aicacia.com".into(),
                description: None,
            },
            client_id: Some(client_id.clone()),
            client_secret: None,
            client_id_issued_at: None,
            client_secret_expires_at: None,
            client_name: "Local Desktop".into(),
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
            allowed_audiences: Vec::new(),
            token_endpoint_auth_method: TokenEndpointAuthMethod::None,
            software_statement: None,
            software_id: None,
            software_version: None,
        };
        self.ensure_setup_public_login_client(request).await
    }

    pub async fn ensure_setup_public_login_client(
        &self,
        request: ClientRegistration,
    ) -> io::Result<ClientRegistration> {
        let _operation = self.setup_operation()?;
        validate_public_login_client(&request)?;
        let client_id = request
            .client_id
            .as_deref()
            .expect("validated login Client ID")
            .to_owned();
        self.begin_client_provisioning_locked(&client_id).await?;
        let registration = match self.oauth2.get_client(&client_id).await {
            Ok(existing) => {
                verify_public_login_registration(&existing, &request)?;
                Ok(existing)
            }
            Err(error) if error.error == idp_model::contract::ErrorCode::InvalidClient => {
                self.create_setup_client(request).await?;
                self.oauth2
                    .get_client(&client_id)
                    .await
                    .map_err(|error| io::Error::other(format!("{error:?}")))
            }
            Err(error) => Err(io::Error::other(format!("{error:?}"))),
        }?;
        self.ensure_client_root_locked(&client_id).await?;
        Ok(registration)
    }

    /// Existing-only validation: missing/revoked or changed Clients are never recreated.
    pub async fn verify_public_login_client(&self, expected: ClientRegistration) -> io::Result<()> {
        validate_public_login_client(&expected)?;
        let id = expected
            .client_id
            .as_deref()
            .expect("validated login Client ID");
        let existing = self
            .oauth2
            .get_client(id)
            .await
            .map_err(|_| invalid("existing public login Client is missing or inactive"))?;
        verify_public_login_registration(&existing, &expected)?;
        self.ensure_client_root_locked(id).await
    }

    /// Caller must prove exact fresh setup intent, never an attempted/historical setup.
    /// Publishes only the preparation boundary; no private material is produced.
    pub async fn prepare_fresh_setup_secret_inventory(
        &self,
        owner: Id,
        clients: &[Id],
    ) -> io::Result<()> {
        let _operation = self.setup_operation()?;
        if owner.is_nil() || clients.iter().any(Id::is_nil) || clients.contains(&owner) {
            return Err(invalid("non-nil distinct setup identities required"));
        }
        let mut unique = clients.to_vec();
        unique.sort();
        unique.dedup();
        if unique.len() != clients.len() {
            return Err(invalid("duplicate Client slots"));
        }
        if self
            .data_dir
            .join("setup-secret-inventory.json")
            .try_exists()?
            || self
                .data_dir
                .join("local-client-key-references")
                .try_exists()?
        {
            return Err(invalid("existing inventory cannot be declared fresh"));
        }
        for client in clients {
            if self
                .current_client_entity(&client.to_string())
                .await?
                .is_some()
            {
                return Err(invalid("existing Client is not fresh"));
            }
        }
        if !self
            .oauth2
            .key_service
            .key_repo()
            .list_by_entity_type_and_id(EntityType::User, owner)
            .await
            .map_err(io::Error::other)?
            .is_empty()
        {
            return Err(invalid("existing User key material is not fresh"));
        }
        self.write_inventory(&json!({"version":3,"issuer":self.oauth2.oauth_config.issuer,"key_namespace":self.key_namespace,"owner":owner,"clients":clients,"phase":"preparing","node":NodePreparation::NeverStarted}))
    }

    fn read_inventory(&self) -> io::Result<JsonValue> {
        let path = self.data_dir.join("setup-secret-inventory.json");
        if !fs::symlink_metadata(&path)?.file_type().is_file() {
            return Err(invalid("inventory must be regular"));
        }
        let record: JsonValue = serde_json::from_slice(&fs::read(path)?)?;
        let owner: Id = serde_json::from_value(record["owner"].clone())?;
        let clients: Vec<Id> = serde_json::from_value(record["clients"].clone())?;
        let mut unique = clients.clone();
        unique.sort();
        unique.dedup();
        if record.as_object().map(|fields| fields.len()) != Some(7)
            || !matches!(record["version"].as_u64(), Some(2 | 3))
            || record["issuer"] != self.oauth2.oauth_config.issuer
            || record["key_namespace"] != self.key_namespace
            || owner.is_nil()
            || clients.iter().any(Id::is_nil)
            || clients.contains(&owner)
            || unique.len() != clients.len()
            || !matches!(
                record["phase"].as_str(),
                Some("preparing" | "ready" | "resetting")
            )
        {
            return Err(invalid(
                "unknown or conflicting secret inventory; explicit recovery required",
            ));
        }
        let node: NodePreparation = serde_json::from_value(record["node"].clone())?;
        validate_node_preparation(&node)?;
        if json!(node) != record["node"]
            || (record["phase"] == "preparing" && !matches!(node, NodePreparation::NeverStarted))
        {
            return Err(invalid("invalid or conflicting Node preparation inventory"));
        }
        Ok(record)
    }

    fn write_inventory(&self, record: &JsonValue) -> io::Result<()> {
        write_durable_json(&self.data_dir.join("setup-secret-inventory.json"), record)?;
        if self.read_inventory()? != *record {
            return Err(invalid("inventory readback changed"));
        }
        Ok(())
    }

    fn ready_inventory(&self) -> io::Result<JsonValue> {
        let record = self.read_inventory()?;
        if record["version"] != 3 {
            return Err(invalid(
                "retained private-attempt inventory requires explicit reset; no metadata mode was inferred",
            ));
        }
        if record["phase"] != "ready" {
            return Err(invalid(
                "initial secret production requires a ready inventory; preparation or reset is pending",
            ));
        }
        Ok(record)
    }

    pub async fn prepare_setup_client_slots(&self, clients: &[Id]) -> io::Result<()> {
        let _operation = self.setup_operation()?;
        if self.read_inventory()?["phase"] == "resetting" {
            return Err(invalid(
                "reset secret cleanup has begun; ordinary preparation is barred",
            ));
        }
        self.prepare_setup_client_slots_locked(clients, false).await
    }

    async fn prepare_setup_client_slots_locked(
        &self,
        clients: &[Id],
        resetting: bool,
    ) -> io::Result<()> {
        let mut inventory = self.read_inventory()?;
        if !resetting && inventory["version"] != 3 {
            return Err(invalid(
                "retained private-attempt inventory requires explicit reset",
            ));
        }
        if inventory["clients"] != json!(clients) {
            return Err(invalid("setup Client slots conflict with inventory"));
        }
        let owner: Id = serde_json::from_value(inventory["owner"].clone())?;
        let mut slots: Vec<String> = clients.iter().map(ToString::to_string).collect();
        slots.push(format!("user:{owner}"));
        for slot in slots {
            let path = self.client_reference_path(&slot)?;
            if !path.try_exists()? {
                if inventory["phase"] != "preparing" {
                    return Err(invalid("lost slot after preparation is unknown history"));
                }
                if !slot.starts_with("user:") && self.current_client_entity(&slot).await?.is_some()
                {
                    return Err(invalid("unexpected Client during verified preparation"));
                }
                self.write_client_reference(
                    &slot,
                    &self.new_client_reference(&slot, "never-started"),
                )?;
            }
            let reference = if resetting {
                self.read_client_reference(&slot)?
            } else {
                self.read_metadata_reference(&slot)?
            };
            if inventory["phase"] == "preparing" && reference["state"] != "never-started" {
                return Err(invalid("secret attempt contradicts preparation boundary"));
            }
        }
        if inventory["phase"] == "preparing" {
            inventory["phase"] = json!("ready");
            self.write_inventory(&inventory)?;
        }
        Ok(())
    }

    async fn current_client_entity(&self, external: &str) -> io::Result<Option<Id>> {
        let results = self
            .engine
            .translate_and_execute(
                &format!(
                    "SELECT id FROM clients WHERE client_id = {}",
                    text(external)
                ),
                &SqlTranslator,
            )
            .await
            .map_err(io::Error::other)?;
        let rows = &results[0].rows;
        if rows.len() > 1 {
            return Err(invalid("ambiguous Client key reference"));
        }
        if let Some(row) = rows.first() {
            let [Value::Uuid(id)] = row.values.as_slice() else {
                return Err(invalid("invalid internal Client identity"));
            };
            if !self
                .engine
                .row_conflicts("clients", &db::Row::new(vec![Value::Uuid(*id)]))
                .await
                .map_err(io::Error::other)?
                .is_empty()
            {
                return Err(invalid("conflicted Client key reference"));
            }
            Ok(Some(*id))
        } else {
            Ok(None)
        }
    }

    pub async fn begin_client_provisioning(&self, external: &str) -> io::Result<()> {
        let _operation = self.setup_operation()?;
        self.begin_client_provisioning_locked(external).await
    }

    async fn begin_client_provisioning_locked(&self, external: &str) -> io::Result<()> {
        let inventory = self.ready_inventory()?;
        if !inventory["clients"]
            .as_array()
            .expect("validated clients")
            .contains(&json!(external))
        {
            return Err(invalid("Client outside setup inventory"));
        }
        let mut reference = self.read_metadata_reference(external)?;
        match reference["state"].as_str() {
            Some("resolved") => {
                let current = self.current_client_entity(external).await?.ok_or_else(|| {
                    invalid("retained setup Client was deleted; explicit reset required")
                })?;
                if reference["entity_id"] != json!(current) {
                    return Err(invalid("Client identity changed"));
                }
                self.verify_local_entity_root(EntityType::Client, current, external)
                    .await
            }
            Some("never-started") => {
                if self.current_client_entity(external).await?.is_some() {
                    return Err(invalid("unexpected existing Client"));
                }
                reference["state"] = json!("pending");
                self.write_client_reference(external, &reference)
            }
            _ => Err(invalid(
                "pending unknown Client history requires explicit recovery",
            )),
        }
    }

    async fn create_setup_client(&self, request: ClientRegistration) -> io::Result<Client> {
        let external = request
            .client_id
            .as_deref()
            .ok_or_else(|| invalid("stable Client ID required"))?
            .to_owned();
        self.oauth2
            .client_repo
            .create_client_with_capture(request, |key, paths| {
                self.capture_initial_root(&external, key, paths)
                    .map_err(|error| RepoError::InvalidInput(error.to_string()))
            })
            .await
            .map_err(io::Error::other)
    }

    pub async fn ensure_setup_infrastructure_client(
        &self,
        request: ClientRegistration,
    ) -> io::Result<ClientRegistration> {
        let _operation = self.setup_operation()?;
        if request.client_type != ClientType::Confidential
            || request.allowed_grant_types != [GrantType::ClientCredentials]
            || request
                .client_secret
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
        {
            return Err(invalid(
                "restricted confidential infrastructure Client required",
            ));
        }
        let external = request
            .client_id
            .as_deref()
            .ok_or_else(|| invalid("stable Client ID required"))?
            .to_owned();
        self.begin_client_provisioning_locked(&external).await?;
        let registration = if self.current_client_entity(&external).await?.is_none() {
            let client = self.create_setup_client(request.clone()).await?;
            let application = self
                .oauth2
                .application_repo
                .find_by_id(client.application_id)
                .await
                .map_err(io::Error::other)?
                .ok_or_else(|| invalid("infrastructure client application is missing"))?;
            if application.name
                != request
                    .application
                    .name
                    .as_deref()
                    .unwrap_or(&request.client_name)
                || application.uri != request.application.uri
                || application.description != request.application.description
            {
                return Err(invalid(
                    "infrastructure client application conflicts with requested registration",
                ));
            }
            let mut registration = ClientRegistration::from(client);
            registration.application = request.application;
            registration.client_secret = request.client_secret;
            registration
        } else {
            self.oauth2
                .ensure_infrastructure_client(request)
                .await
                .map_err(|error| io::Error::other(format!("{error:?}")))?
        };
        self.ensure_client_root_locked(&external).await?;
        Ok(registration)
    }

    /// Existing-only public relationship evidence; never imports a verifier or repairs a Client.
    pub async fn verify_setup_infrastructure_registration(
        &self,
        expected: &ClientRegistration,
    ) -> io::Result<()> {
        if expected.client_type != ClientType::Confidential
            || expected.client_secret.is_some()
            || expected.allowed_grant_types != [GrantType::ClientCredentials]
        {
            return Err(invalid(
                "public confidential relationship evidence required",
            ));
        }
        let external = expected
            .client_id
            .as_deref()
            .ok_or_else(|| invalid("stable Client ID required"))?;
        let client = self
            .oauth2
            .client_repo
            .find_client_by_client_id(external)
            .await
            .map_err(io::Error::other)?
            .ok_or_else(|| invalid("registered service Client is missing or inactive"))?;
        let application = self
            .oauth2
            .application_repo
            .find_by_id(client.application_id)
            .await
            .map_err(io::Error::other)?
            .ok_or_else(|| invalid("service Application is missing"))?;
        let mut current = ClientRegistration::from(client);
        current.application = ApplicationRegistration {
            name: Some(application.name),
            uri: application.uri,
            description: application.description,
        };
        if current != *expected {
            return Err(invalid("registered service Client metadata changed"));
        }
        self.ensure_client_root_locked(external).await
    }

    fn client_reference_path(&self, external: &str) -> io::Result<std::path::PathBuf> {
        let identity = external.strip_prefix("user:").unwrap_or(external);
        let id: Id = identity.parse().map_err(io::Error::other)?;
        if id.is_nil() || id.to_string() != identity {
            return Err(invalid("canonical stable setup Client ID required"));
        }
        Ok(self
            .data_dir
            .join("local-client-key-references")
            .join(format!("{}.json", external.replace(':', "-"))))
    }

    fn new_client_reference(&self, external: &str, state: &str) -> JsonValue {
        json!({"version":3,"issuer":self.oauth2.oauth_config.issuer,"key_namespace":self.key_namespace,"slot":external,"mode":"metadata-root","state":state,"entity_id":null,"root_id":null,"paths":[]})
    }

    fn read_client_reference(&self, external: &str) -> io::Result<JsonValue> {
        let path = self.client_reference_path(external)?;
        if !fs::symlink_metadata(&path)?.file_type().is_file() {
            return Err(invalid("Client key reference is not a regular file"));
        }
        let record: JsonValue = serde_json::from_slice(&fs::read(path)?)?;
        let metadata_root = record["version"] == 3;
        let shape = if metadata_root {
            record.as_object().map(|fields| fields.len()) == Some(9)
                && record["mode"] == "metadata-root"
        } else {
            record["version"] == 2 && record.as_object().map(|fields| fields.len()) == Some(7)
        };
        if !shape
            || record["issuer"] != self.oauth2.oauth_config.issuer
            || record["key_namespace"] != self.key_namespace
            || record["slot"] != external
            || !matches!(
                record["state"].as_str(),
                Some("never-started" | "pending" | "resolved")
            )
        {
            return Err(invalid(
                "invalid or conflicting local Client key references; reset remains incomplete",
            ));
        }
        let paths: Vec<String> = serde_json::from_value(record["paths"].clone())?;
        let parsed: Vec<key::DerivationPath> = paths
            .iter()
            .map(|path| path.parse().map_err(io::Error::other))
            .collect::<io::Result<_>>()?;
        if record["state"] == "resolved" {
            if parsed.len() < 2
                || parsed.last() != Some(&key::DerivationPath::default())
                || parsed
                    .windows(2)
                    .any(|pair| pair[0].parent().as_ref() != Some(&pair[1]))
                || parsed
                    .iter()
                    .zip(&paths)
                    .any(|(path, text)| path.to_string() != *text)
            {
                return Err(invalid("incomplete initial private path ancestry"));
            }
            let id: Id = serde_json::from_value(record["entity_id"].clone())?;
            if metadata_root {
                let root: Id = serde_json::from_value(record["root_id"].clone())?;
                if root.is_nil() {
                    return Err(invalid("captured metadata root identity is missing"));
                }
            }
            if id.is_nil() || record["paths"].as_array().is_none_or(Vec::is_empty) {
                return Err(invalid("incomplete retained initial inventory"));
            }
        } else if !record["entity_id"].is_null()
            || !paths.is_empty()
            || (metadata_root && !record["root_id"].is_null())
        {
            return Err(invalid(
                "unresolved Client contains an unverified internal identity",
            ));
        }
        Ok(record)
    }

    fn write_client_reference(&self, external: &str, record: &JsonValue) -> io::Result<()> {
        let path = self.client_reference_path(external)?;
        let directory = path.parent().expect("reference directory");
        fs::create_dir_all(directory)?;
        if !fs::symlink_metadata(directory)?.file_type().is_dir() {
            return Err(invalid("Client reference directory must be local"));
        }
        fs::File::open(&self.data_dir)?.sync_all()?;
        if path.try_exists()? {
            self.read_client_reference(external)?;
        }
        write_durable_json(&path, record)?;
        if self.read_client_reference(external)? != *record {
            return Err(invalid("Client key reference readback changed"));
        }
        Ok(())
    }

    fn read_metadata_reference(&self, slot: &str) -> io::Result<JsonValue> {
        let record = self.read_client_reference(slot)?;
        if record["version"] != 3 || record["mode"] != "metadata-root" {
            return Err(invalid(
                "retained private-attempt reference requires explicit reset; no metadata mode was inferred",
            ));
        }
        Ok(record)
    }

    fn capture_initial_root(
        &self,
        slot: &str,
        key: &Key,
        paths: &[DerivationPath],
    ) -> io::Result<()> {
        self.ready_inventory()?;
        let mut record = self.read_metadata_reference(slot)?;
        if record["state"] != "pending"
            || key.entity_id.is_nil()
            || key.id.is_nil()
            || paths.is_empty()
        {
            return Err(invalid(
                "initial root capture conflicts with pending intent",
            ));
        }
        record["state"] = json!("resolved");
        record["entity_id"] = json!(key.entity_id);
        record["root_id"] = json!(key.id);
        record["paths"] = json!(paths.iter().map(ToString::to_string).collect::<Vec<_>>());
        self.write_client_reference(slot, &record)
    }

    pub fn prepare_node_signer(&self, binding: &NodeSignerBinding) -> io::Result<JwkPublic> {
        let _operation = self.setup_operation()?;
        self.prepare_node_signer_locked(binding)
    }

    fn prepare_node_signer_locked(&self, binding: &NodeSignerBinding) -> io::Result<JwkPublic> {
        let mut inventory = self.ready_inventory()?;
        validate_binding(binding)?;
        let node: NodePreparation = serde_json::from_value(inventory["node"].clone())?;
        if !matches!(node, NodePreparation::NeverStarted) {
            return self.verify_prepared_node_signer_locked(binding);
        }
        let keys = &self.oauth2.key_service;
        let namespace = format!(
            "{}:node:{}:{}",
            self.key_namespace, binding.member_id, binding.key_id
        );
        if keys
            .private_key_repo()
            .load(&namespace, &DerivationPath::default())
            .map_err(io::Error::other)?
            .is_some()
        {
            inventory["node"] = json!(NodePreparation::Attempted {
                binding: binding.clone(),
                public_signer: None
            });
            self.write_inventory(&inventory)?;
            return Err(invalid(
                "unexpected existing Node private material contradicts fresh preparation; explicit recovery required",
            ));
        }
        let entropy = MasterKey::entropy().map_err(io::Error::other)?;
        let private: DerivedKey = MasterKey::from_entropy(entropy)
            .map_err(io::Error::other)?
            .into();
        let public = node_signer_public(binding, &private)?;
        inventory["node"] = json!(NodePreparation::Attempted {
            binding: binding.clone(),
            public_signer: Some(public)
        });
        self.write_inventory(&inventory)?;
        keys.private_key_repo()
            .store(&namespace, &private)
            .map_err(io::Error::other)?;
        self.verify_prepared_node_signer_locked(binding)
    }

    /// Existing-only local preparation verification; never authorizes trust or issuance.
    pub fn verify_prepared_node_signer(
        &self,
        binding: &NodeSignerBinding,
    ) -> io::Result<JwkPublic> {
        let _operation = self.setup_operation()?;
        self.verify_prepared_node_signer_locked(binding)
    }

    fn verify_prepared_node_signer_locked(
        &self,
        binding: &NodeSignerBinding,
    ) -> io::Result<JwkPublic> {
        let mut inventory = self.ready_inventory()?;
        validate_binding(binding)?;
        let node: NodePreparation = serde_json::from_value(inventory["node"].clone())?;
        let prepared = matches!(&node, NodePreparation::Prepared { .. });
        let (retained, expected) = match node {
            NodePreparation::Attempted {
                binding,
                public_signer: Some(public),
            }
            | NodePreparation::Prepared {
                binding,
                public_signer: public,
            } => (binding, public),
            _ => {
                return Err(invalid(
                    "no exact attempted Node preparation is retained; no private material was created",
                ));
            }
        };
        if json!(retained) != json!(binding) {
            return Err(invalid(
                "Node binding conflicts with retained preparation; no private material was created",
            ));
        }
        let namespace = format!(
            "{}:node:{}:{}",
            self.key_namespace, binding.member_id, binding.key_id
        );
        let private = self.oauth2.key_service.private_key_repo().load(&namespace, &DerivationPath::default()).map_err(io::Error::other)?.ok_or_else(|| invalid("attempted Node private signer is missing; explicit recovery required, no signer was recreated"))?;
        let public = node_signer_public(binding, &private)?;
        if public != expected {
            return Err(invalid(
                "Node private signer conflicts with captured public material; no signer was changed",
            ));
        }
        if !prepared {
            inventory["node"] = json!(NodePreparation::Prepared {
                binding: binding.clone(),
                public_signer: public.clone()
            });
            self.write_inventory(&inventory)?;
        }
        Ok(public)
    }

    pub async fn verify_node_signer(&self, binding: &NodeSignerBinding) -> io::Result<JwkPublic> {
        validate_binding(binding)?;
        self.oauth2
            .key_service
            .load_node_signer(binding, &self.oauth2.oauth_config.issuer, now()?)
            .await
            .map(JwkPublic::from)
            .map_err(io::Error::other)
    }

    pub async fn delete_local_secrets(
        &self,
        member_id: Id,
        key_id: Id,
        owner: Id,
        clients: &[Id],
    ) -> io::Result<()> {
        let _operation = self.setup_operation()?;
        if member_id.is_nil() || key_id.is_nil() || owner.is_nil() || clients.iter().any(Id::is_nil)
        {
            return Err(invalid(
                "local secret deletion requires persisted non-nil IDs",
            ));
        }
        let inventory = self.read_inventory()?;
        if inventory["owner"] != json!(owner) || inventory["clients"] != json!(clients) {
            return Err(invalid("reset identities conflict with secret inventory"));
        }
        let node: NodePreparation = serde_json::from_value(inventory["node"].clone())?;
        match &node {
            NodePreparation::Attempted { binding, .. }
            | NodePreparation::Prepared { binding, .. }
                if binding.member_id != member_id || binding.key_id != key_id =>
            {
                return Err(invalid(
                    "reset Node identities conflict with retained preparation",
                ));
            }
            _ => (),
        }
        self.prepare_setup_client_slots_locked(clients, true)
            .await?;
        let keys = &self.oauth2.key_service;
        let mut entities = Vec::with_capacity(clients.len() + 1);
        let mut slots: Vec<(String, EntityType)> = clients
            .iter()
            .map(|id| (id.to_string(), EntityType::Client))
            .collect();
        slots.push((format!("user:{owner}"), EntityType::User));
        for (slot, entity_type) in slots {
            let reference = self.read_client_reference(&slot)?;
            let current = if entity_type == EntityType::Client {
                self.current_client_entity(&slot).await?
            } else {
                None
            };
            if reference["state"] == "never-started" {
                if current.is_some() {
                    return Err(invalid(
                        "unexpected entity contradicts never-started inventory",
                    ));
                }
                if entity_type == EntityType::User {
                    let retained = self.engine.translate_and_execute(
                        &format!("SELECT id FROM keys WHERE entity_type = {} AND entity_id = CAST('{}' AS UUID)", entity_type as i64, owner),
                        &SqlTranslator,
                    ).await.map_err(io::Error::other)?;
                    if !retained[0].rows.is_empty() {
                        return Err(invalid(
                            "unexpected User metadata contradicts never-started inventory",
                        ));
                    }
                    let namespace = keys.scoped_namespace(entity_type, owner);
                    if keys
                        .private_key_repo()
                        .load(&namespace, &DerivationPath::default())
                        .map_err(io::Error::other)?
                        .is_some()
                    {
                        return Err(invalid(
                            "unexpected User private material contradicts never-started inventory",
                        ));
                    }
                }
                continue;
            }
            if reference["state"] != "resolved" {
                return Err(invalid(
                    "pending initial secret inventory is unknown; reset remains incomplete",
                ));
            }
            let entity: Id = serde_json::from_value(reference["entity_id"].clone())?;
            if current.is_some_and(|id| id != entity)
                || (entity_type == EntityType::User && entity != owner)
            {
                return Err(invalid("entity conflicts with captured inventory"));
            }
            let retained = self.engine.translate_and_execute(
                &format!("SELECT derivation_path FROM keys WHERE entity_type = {} AND entity_id = CAST('{}' AS UUID)", entity_type as i64, entity),
                &SqlTranslator,
            ).await.map_err(io::Error::other)?;
            let mut texts: Vec<String> = serde_json::from_value(reference["paths"].clone())?;
            texts.push(DerivationPath::default().to_string());
            for row in &retained[0].rows {
                let [Value::Text(path)] = row.values.as_slice() else {
                    return Err(invalid(
                        "invalid retained private-key path metadata; reset remains incomplete",
                    ));
                };
                texts.push(path.clone());
            }
            let mut paths = Vec::new();
            for text in texts {
                let path: DerivationPath = text.parse().map_err(io::Error::other)?;
                if path.to_string() != text {
                    return Err(invalid(
                        "noncanonical retained private-key path; reset remains incomplete",
                    ));
                }
                let mut current = Some(path);
                while let Some(path) = current {
                    current = path.parent();
                    paths.push(path);
                }
            }
            paths.sort_by_key(ToString::to_string);
            paths.dedup();
            let metadata_only = reference["version"] == 3;
            let namespace = keys.scoped_namespace(entity_type, entity);
            for path in &paths {
                let private = keys
                    .private_key_repo()
                    .load(&namespace, path)
                    .map_err(io::Error::other)?;
                if metadata_only && private.is_some() {
                    return Err(invalid(
                        "unexpected private material contradicts metadata-only capture; explicit recovery required before reset",
                    ));
                }
            }
            entities.push((entity_type, entity, paths, metadata_only));
        }
        let node_namespace = format!("{}:node:{}:{}", self.key_namespace, member_id, key_id);
        let node_path = DerivationPath::default();
        let private = keys
            .private_key_repo()
            .load(&node_namespace, &node_path)
            .map_err(io::Error::other)?;
        if inventory["version"] == 3
            && matches!(node, NodePreparation::NeverStarted)
            && private.is_some()
        {
            return Err(invalid(
                "unexpected Node private material contradicts never-started inventory",
            ));
        }
        let mut inventory = self.read_inventory()?;
        inventory["phase"] = json!("resetting");
        self.write_inventory(&inventory)?;
        for (entity_type, entity, paths, metadata_only) in &entities {
            let namespace = keys.scoped_namespace(*entity_type, *entity);
            for path in paths {
                if *metadata_only {
                    if keys
                        .private_key_repo()
                        .load(&namespace, path)
                        .map_err(io::Error::other)?
                        .is_some()
                    {
                        return Err(invalid(
                            "private material appeared during metadata-only reset; evidence retained",
                        ));
                    }
                } else {
                    delete_verified(keys.private_key_repo(), &namespace, path)?;
                }
            }
        }
        delete_verified(keys.private_key_repo(), &node_namespace, &node_path)?;
        Ok(())
    }

    /// Restricted owner-local fresh setup; never exposed as an unauthenticated HTTP mutation.
    pub async fn ensure_initial_node(
        &self,
        binding: &NodeSignerBinding,
        owner: Id,
        name: &str,
    ) -> io::Result<()> {
        let _operation = self.setup_operation()?;
        if self.ready_inventory()?["owner"] != json!(owner) {
            return Err(invalid("initial Node owner conflicts with setup inventory"));
        }
        validate_binding(binding)?;
        if owner.is_nil() || name.trim().is_empty() {
            return Err(invalid("initial Node owner and name are required"));
        }
        let rows = self
            .engine
            .translate_and_execute("SELECT id FROM users", &SqlTranslator)
            .await
            .map_err(io::Error::other)?;
        if rows[0].rows.len() != 1 || rows[0].rows[0].values != [Value::Uuid(owner)] {
            return Err(invalid("fresh setup requires the exact initial local User"));
        }
        let enrolled = self
            .engine
            .translate_and_execute("SELECT member_id FROM idp_replica_members", &SqlTranslator)
            .await
            .map_err(io::Error::other)?;
        let public = if enrolled[0].rows.is_empty() {
            self.prepare_node_signer_locked(binding)?
        } else {
            let prepared = self.verify_prepared_node_signer_locked(binding)?;
            let approved = self.verify_node_signer(binding).await?;
            if prepared != approved {
                return Err(invalid(
                    "prepared Node signer conflicts with approved trust",
                ));
            }
            approved
        };
        let issuer = self.oauth2.oauth_config.issuer.clone();
        DbReplicaSignerRepo::new(self.engine.clone())
            .enroll(
                &ReplicaMembership {
                    installation_id: issuer.clone(),
                    member_id: binding.member_id,
                    endpoint_id: binding.endpoint_id.clone(),
                    issuer: issuer.clone(),
                    approved_at: 1,
                    revoked_at: None,
                },
                &IdpSignerRecord {
                    member_id: binding.member_id,
                    key_id: binding.key_id,
                    issuer,
                    public_jwk: public,
                    approved_at: Some(1),
                    revoked_at: None,
                    expires_at: None,
                },
            )
            .await
            .map_err(io::Error::other)?;
        self.verify_node_signer(binding).await?;
        Ok(())
    }
}

fn validate_metadata_root(key: &Key, entity: EntityType, id: Id) -> io::Result<()> {
    let path = key.derivation_path().map_err(io::Error::other)?;
    if id.is_nil()
        || key.id.is_nil()
        || key.entity_id.is_nil()
        || key.entity_type != entity
        || key.entity_id != id
        || key.parent_id.is_some()
        || key.public_jwk.is_some()
        || !key.hardened
        || path.to_string() != key.derivation_path
        || key.derivation_path != Key::build_derivation_path(None, key.derivation_index, true)
    {
        return Err(invalid(
            "invalid Principal metadata-root shape; explicit recovery required",
        ));
    }
    Ok(())
}

fn validate_node_preparation(node: &NodePreparation) -> io::Result<()> {
    let (binding, public) = match node {
        NodePreparation::NeverStarted => return Ok(()),
        NodePreparation::Attempted {
            binding,
            public_signer,
        } => (binding, public_signer.as_ref()),
        NodePreparation::Prepared {
            binding,
            public_signer,
        } => (binding, Some(public_signer)),
    };
    validate_binding(binding)?;
    if let Some(public) = public
        && (public.kid != binding.key_id.to_string()
            || public.r#use != KeyUse::Signature
            || public.alg != JwsAlgorithm::ES256K
            || !matches!(&public.params, JwkPublicParameters::Ec { crv, .. } if crv == "secp256k1"))
    {
        return Err(invalid("invalid captured Node public material"));
    }
    Ok(())
}

fn node_signer_public(binding: &NodeSignerBinding, private: &DerivedKey) -> io::Result<JwkPublic> {
    let (x, y) = private.to_xy().map_err(io::Error::other)?;
    Ok(JwkPublic {
        kid: binding.key_id.to_string(),
        r#use: KeyUse::Signature,
        alg: JwsAlgorithm::ES256K,
        params: JwkPublicParameters::Ec {
            crv: "secp256k1".into(),
            x: STANDARD_NO_PAD.encode(x),
            y: STANDARD_NO_PAD.encode(y),
        },
    })
}

fn write_durable_json(path: &std::path::Path, record: &JsonValue) -> io::Result<()> {
    let directory = path.parent().expect("inventory directory");
    fs::create_dir_all(directory)?;
    if !fs::symlink_metadata(directory)?.file_type().is_dir() {
        return Err(invalid("inventory directory must be local"));
    }
    let temporary = directory.join(format!(".{}.tmp", Id::now_v7()));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary)?;
    file.write_all(&serde_json::to_vec(record)?)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    for ancestor in fs::canonicalize(directory)?.ancestors() {
        fs::File::open(ancestor)?.sync_all()?;
    }
    Ok(())
}

fn delete_verified<P: PrivateKeyRepo>(
    repo: &P,
    namespace: &str,
    path: &key::DerivationPath,
) -> io::Result<()> {
    if repo
        .load(namespace, path)
        .map_err(io::Error::other)?
        .is_some()
    {
        repo.delete(namespace, path).map_err(io::Error::other)?;
        if repo
            .load(namespace, path)
            .map_err(io::Error::other)?
            .is_some()
        {
            return Err(io::Error::other(
                "secure-store deletion was not durable; retry local reset",
            ));
        }
    }
    Ok(())
}

fn validate_public_login_client(request: &ClientRegistration) -> io::Result<()> {
    let id = request
        .client_id
        .as_deref()
        .ok_or_else(|| invalid("stable login Client ID required"))?;
    let parsed: Id = id.parse().map_err(io::Error::other)?;
    if parsed.is_nil()
        || parsed.to_string() != id
        || request.client_type != ClientType::Public
        || request.profile != ClientProfile::Native
        || request.token_endpoint_auth_method != TokenEndpointAuthMethod::None
        || request.client_secret.is_some()
        || request.allowed_grant_types
            != [
                GrantType::Password,
                GrantType::RefreshToken,
                GrantType::TokenExchange,
            ]
        || request.allowed_scopes != ["openid", "profile", "email"]
        || !request.redirect_uris.is_empty()
        || !request.response_types.is_empty()
    {
        return Err(invalid("restricted public login Client required"));
    }
    Ok(())
}

fn verify_public_login_registration(
    existing: &ClientRegistration,
    expected: &ClientRegistration,
) -> io::Result<()> {
    if existing.client_id != expected.client_id
        || existing.application.uri != expected.application.uri
        || existing.client_name != expected.client_name
        || existing.client_type != expected.client_type
        || existing.profile != expected.profile
        || existing.token_endpoint_auth_method != expected.token_endpoint_auth_method
        || existing.allowed_grant_types != expected.allowed_grant_types
        || existing.allowed_scopes != expected.allowed_scopes
        || existing.allowed_audiences != expected.allowed_audiences
        || existing.redirect_uris != expected.redirect_uris
        || !existing.response_types.is_empty()
        || existing.client_secret.is_some()
    {
        return Err(invalid(
            "existing public login Client conflicts with restricted setup; no Client was changed",
        ));
    }
    Ok(())
}

fn validate_binding(binding: &NodeSignerBinding) -> io::Result<()> {
    let endpoint = binding
        .endpoint_id
        .parse::<EndpointId>()
        .map_err(io::Error::other)?;
    if binding.member_id.is_nil()
        || binding.key_id.is_nil()
        || endpoint.to_string() != binding.endpoint_id
    {
        return Err(invalid("invalid Node signer binding"));
    }
    Ok(())
}

fn text(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn now() -> io::Result<i64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs()
        .try_into()
        .map_err(io::Error::other)
}
