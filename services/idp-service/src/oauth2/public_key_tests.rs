use super::{
    DbApplicationRepo, DbClientRepo, DbKeyRepo, DbOAuth2AuthorizationCodeRepo,
    DbOAuth2RefreshTokenRepo, DbOAuth2UserConsentRepo, DbUserRepo, KeyService, OAuth2Config,
    OAuth2Service, PasswordConfig, TestPrivateKeyRepo, TestService, auth, grant, issue, setup, up,
};
type TestEngine = ofdb_sql::Database;
use crate::oauth2::jwt::{decode_jwt, encode_jwt, verify_jwt};
use crate::repo::{KeyRepo, UserRepo};
use of_model::contract::standard_claims::StandardClaims;
use of_model::id::Id;
use of_model::idp::contract::entity_type::EntityType;
use ofdb_sql::Query;
use ofdb_sql::QueryColumn;
use ofdb_sql::QueryExpr;
use ofdb_sql::QueryExprValue;
use ofdb_sql::QueryFrom;
use ofdb_sql::QueryUpdate;
use ofdb_sql::QueryUpdateAssignment;
use ofdb_sql::SqlTranslator;
use ofdb_sql::Statement;
use ofdb_sql::Value;
use std::sync::Arc;

async fn public_verifier(
    source: &TestEngine,
    authority: &TestService,
) -> (Arc<TestEngine>, TestService) {
    let engine = Arc::new(ofdb_sql::Database::in_memory());
    let jwks = authority
        .list_jwks()
        .await
        .expect("snapshot of issuer JWKS");
    assert!(
        !crate::security_sync::public_security_tables()
            .iter()
            .any(|table| table.name() == "issuer_keys")
    );
    let mut history = source.export_history().await.expect("read fixture history");
    history.retain(|unit| {
        let ofdb_sql::SyncKey::Row { table, .. } = &unit.key;
        !matches!(
            table.as_str(),
            "issuer_keys" | "local_client_secret_verifiers" | "local_user_password_verifiers"
        )
    });
    engine
        .import_history(history)
        .await
        .expect("import public verifier fixture");
    let keys = Arc::new(KeyService::new(
        DbKeyRepo::new(engine.client()),
        TestPrivateKeyRepo::default(),
        "independent-node",
    ));
    let service = OAuth2Service::new(
        DbApplicationRepo::new(engine.client()),
        DbClientRepo::new(engine.client(), Arc::clone(&keys)),
        DbOAuth2AuthorizationCodeRepo::new(engine.client()),
        DbOAuth2RefreshTokenRepo::new(engine.client()),
        DbUserRepo::new(engine.client(), PasswordConfig::default()),
        DbOAuth2UserConsentRepo::new(engine.client()),
        keys,
        OAuth2Config {
            trusted_issuer_jwks: Some(jwks),
            ..OAuth2Config::default()
        },
    );
    (engine, service)
}

async fn set(engine: &TestEngine, table: &str, id: Id, name: &str, value: Value) {
    let column = |name: &str| QueryColumn::new(table.into(), name.into());
    engine
        .client()
        .execute(vec![Statement::Query(Query::Update(QueryUpdate {
            from: QueryFrom {
                table: table.into(),
                joins: vec![],
            },
            assignments: vec![QueryUpdateAssignment {
                column: column(name),
                value: QueryExprValue::Value(value),
            }],
            predicate: Some(QueryExpr::Equals(
                Box::new(QueryExpr::Value(QueryExprValue::Column(column("id")))),
                Box::new(QueryExpr::Value(QueryExprValue::Value(Value::Uuid(id)))),
            )),
            returning: None,
        }))])
        .await
        .expect("change security fixture");
}

#[tokio::test]
async fn access_jwts_verify_with_retained_issuer_keys_without_identity_or_private_material() {
    let (engine, service, client, principal) = setup().await;
    let token = issue(&service, &client, &principal).await.access_token.0;
    let (verifier_engine, verifier) = public_verifier(&engine, &service).await;
    verifier_engine
        .client()
        .translate_and_execute(
            "DROP TABLE users; DROP TABLE clients; DROP TABLE keys",
            &SqlTranslator,
        )
        .await
        .expect("remove live identity tables from verifier");
    let claims = verifier
        .validate_access_token(&token, &client.client_id)
        .await
        .expect("verify offline with public trust");
    assert_eq!(claims.sub, principal.user.id.to_string());
    let access = verifier
        .verify_access_token_claims(&token)
        .await
        .expect("verified access claims");
    assert_eq!(access.application_id, client.application_id);
    let retained_jwks = service.list_jwks().await.expect("issuer JWKS");
    let now = chrono::Utc::now().timestamp();
    let access = crate::oauth2::verify_access_jwt(
        &retained_jwks,
        &service.oauth_config.issuer,
        &client.client_id,
        &token,
        now,
    )
    .expect("retained JWKS needs no repositories");
    assert_eq!(access.application_id, client.application_id);
    assert!(
        verifier
            .key_service
            .private_key_repo()
            .0
            .lock()
            .expect("local key store")
            .is_empty()
    );
    assert!(
        verifier
            .validate_access_token(&token, "wrong-audience")
            .await
            .is_err()
    );
    assert!(
        verifier.find_public_jwk(principal.key.id).await.is_err(),
        "User authentication key is not an issuer signer"
    );
    assert_eq!(
        verifier
            .list_jwks()
            .await
            .expect("public node JWKS")
            .keys
            .len(),
        1
    );
    let binding = service
        .oauth_config
        .local_signer
        .as_ref()
        .expect("source binding");
    assert!(
        verifier
            .key_service
            .load_node_signer(
                binding,
                &service.oauth_config.issuer,
                chrono::Utc::now().timestamp()
            )
            .await
            .is_err(),
        "approved public trust is not private capability"
    );
    let different_public = verifier
        .key_service
        .prepare_node_signer(binding)
        .expect("prepare independent local material without approving it");
    assert_ne!(
        different_public,
        service
            .find_public_jwk(binding.key_id)
            .await
            .expect("approved public material")
    );
    assert!(
        verifier
            .key_service
            .load_node_signer(
                binding,
                &service.oauth_config.issuer,
                chrono::Utc::now().timestamp()
            )
            .await
            .is_err(),
        "a different local private signer cannot impersonate an approved signer"
    );

    let (_, original) = decode_jwt::<StandardClaims>(&token).expect("claims");
    for field in [
        "issuer",
        "audience",
        "purpose",
        "expired",
        "not yet valid",
        "future issuance",
    ] {
        let mut claims = original.clone();
        match field {
            "issuer" => claims.iss = "https://other-installation.example".into(),
            "audience" => claims.aud = "another-api".into(),
            "purpose" => claims.r#use = of_model::contract::token_use::TokenUse::Id,
            "expired" => claims.exp = now,
            "not yet valid" => claims.nbf = now + 300,
            "future issuance" => claims.iat = now + 300,
            _ => unreachable!(),
        }
        let invalid = encode_jwt(
            &service.local_signing_jwk().await.expect("local signer"),
            &crate::oauth2::token::AccessClaims {
                standard_claims: claims,
                application_id: client.application_id,
                introspection_audiences: vec![],
            },
        )
        .expect("sign invalid claims");
        assert!(
            crate::oauth2::verify_access_jwt(
                &retained_jwks,
                &service.oauth_config.issuer,
                &client.client_id,
                &invalid,
                now,
            )
            .is_err(),
            "accepted {field}"
        );
    }
    let (signing_input, _) = token.rsplit_once('.').expect("JWT signature separator");
    let invalid = format!("{signing_input}.{}", "A".repeat(86));
    assert!(
        crate::oauth2::verify_access_jwt(
            &retained_jwks,
            &service.oauth_config.issuer,
            &client.client_id,
            &invalid,
            now,
        )
        .is_err()
    );

    use base64::{
        Engine,
        engine::general_purpose::{STANDARD_NO_PAD, URL_SAFE_NO_PAD},
    };
    use k256::ecdsa::{Signature, SigningKey, signature::Signer};
    use of_model::idp::contract::jwk_private_parameters::JwkPrivateParameters;

    let signer = service.local_signing_jwk().await.expect("local signer");
    let JwkPrivateParameters::Ec { d, .. } = signer.params else {
        panic!("EC issuer key")
    };
    let key = SigningKey::from_slice(&STANDARD_NO_PAD.decode(d).expect("EC scalar"))
        .expect("issuer signing key");
    let (_, payload) = signing_input
        .split_once('.')
        .expect("JWT payload separator");
    let header = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&serde_json::json!({
            "kid": signer.kid, "typ": "JWT", "alg": "ES256",
        }))
        .expect("wrong algorithm header"),
    );
    let wrong_input = format!("{header}.{payload}");
    let signature: Signature = key.sign(wrong_input.as_bytes());
    let invalid = format!(
        "{wrong_input}.{}",
        URL_SAFE_NO_PAD.encode(signature.to_bytes())
    );
    assert!(
        crate::oauth2::verify_access_jwt(
            &retained_jwks,
            &service.oauth_config.issuer,
            &client.client_id,
            &invalid,
            now,
        )
        .is_err(),
        "reject a wrong algorithm even with a correct signature"
    );
}

#[tokio::test]
async fn token_verification_requires_no_management_device_tables() {
    let (source, service, client, principal) = setup().await;
    let token = issue(&service, &client, &principal).await.access_token.0;
    let (engine, verifier) = public_verifier(&source, &service).await;
    engine
        .client()
        .execute_sql("DROP TABLE devices", None)
        .await
        .expect("remove the unused Device table from the shared fixture schema");
    assert!(
        engine
            .client()
            .translate_and_execute("SELECT * FROM devices", &SqlTranslator)
            .await
            .is_err(),
        "Management Device state is not an IdP schema prerequisite"
    );
    verifier
        .validate_access_token(&token, &client.client_id)
        .await
        .expect("public signer and Principal trust without Device tables");
    assert_eq!(verifier.list_jwks().await.expect("IdP JWKS").keys.len(), 1);
}

#[tokio::test]
async fn only_issuer_trust_changes_invalidate_existing_access_tokens() {
    let (source, service, client, principal) = setup().await;
    let token = issue(&service, &client, &principal).await.access_token.0;
    for kind in ["withdraw issuer key", "client", "principal", "corrupt-key"] {
        let (engine, mut verifier) = public_verifier(&source, &service).await;
        assert!(
            verifier
                .validate_access_token(&token, &client.client_id)
                .await
                .is_ok()
        );
        match kind {
            "withdraw issuer key" => verifier
                .oauth_config
                .trusted_issuer_jwks
                .as_mut()
                .expect("retained keys")
                .keys
                .clear(),
            "client" => {
                set(
                    &engine,
                    "clients",
                    client.id,
                    "revoked_at",
                    Value::Integer(1),
                )
                .await
            }
            "principal" => {
                verifier
                    .user_repo
                    .delete_user_by_id(principal.user.id)
                    .await
                    .expect("remove locally active Principal");
            }
            _ => {
                set(
                    &engine,
                    "keys",
                    principal.key.id,
                    "public_jwk",
                    Value::Text("invalid".into()),
                )
                .await
            }
        }
        assert_eq!(
            verifier
                .validate_access_token(&token, &client.client_id)
                .await
                .is_err(),
            kind == "withdraw issuer key",
            "unexpected access result for {kind}"
        );
    }
}

#[tokio::test]
async fn public_keys_survive_native_reopen_without_secret_store() {
    let path = std::env::temp_dir().join(format!("of-public-key-{}.redb", Id::now_v7()));
    let entity_id = Id::now_v7();
    let (id, public, token) = {
        let engine = Arc::new(ofdb_sql::Database::open(&path).expect("open temporary database"));
        up(&engine.client())
            .await
            .expect("initialize current schema");
        let keys = KeyService::new(
            DbKeyRepo::new(engine.client()),
            TestPrivateKeyRepo::default(),
            "native-signer",
        );
        keys.ensure_entity_master_key(EntityType::User, entity_id, "local-test-passphrase")
            .expect("create local master key");
        let (key, private) = keys
            .create_key(
                None,
                EntityType::User,
                entity_id,
                true,
                "native".into(),
                None,
            )
            .await
            .expect("persist key and public material");
        let token = encode_jwt(
            &key.to_jwk_private(&private).expect("private signing JWK"),
            &serde_json::json!({"sub": entity_id.to_string()}),
        )
        .expect("sign test token");
        (key.id, key.public_jwk.expect("public material"), token)
    };
    {
        let engine = Arc::new(
            ofdb_sql::Database::open(&path).expect("reopen database without private store"),
        );
        let repo = DbKeyRepo::new(engine.client());
        let key = repo
            .find_by_id(id)
            .await
            .expect("read persisted key")
            .expect("active key");
        let stored = key.public_jwk.expect("public material survives reopen");
        assert_eq!(stored, public);
        let (_, claims) = verify_jwt::<serde_json::Value>(&stored, &token)
            .expect("verify with persisted public material only");
        assert_eq!(claims["sub"], entity_id.to_string());
        let json = serde_json::to_value(&stored).expect("serialize public material");
        assert!(
            json.get("d").is_none(),
            "public record must not contain private scalar"
        );
    }
    std::fs::remove_file(path).expect("remove temporary database");
}

#[tokio::test]
async fn id_tokens_bind_the_principal_root_for_owner_local_verification() {
    let (engine, service, client, principal) = setup().await;
    let tokens = issue(&service, &client, &principal).await;
    let token = tokens.id_token.expect("issued ID token").0;
    let (header, claims) = decode_jwt::<serde_json::Value>(&token).expect("ID claims");
    assert_eq!(claims["principal_key_id"], principal.key.id.to_string());
    assert_ne!(header.kid, principal.key.id.to_string());
    let (_, typed) = decode_jwt::<of_model::idp::contract::id_token_claims::IdTokenClaims>(&token)
        .expect("typed bound ID claims");
    assert_eq!(typed.principal_key_id, principal.key.id);
    assert_eq!(
        typed.user_info.as_ref().expect("OIDC profile").sub,
        principal.user.id.to_string()
    );
    assert_eq!(
        serde_json::to_value(&typed).expect("roundtrip profile"),
        claims
    );
    let (verifier_engine, verifier) = public_verifier(&engine, &service).await;
    let (verified, _) = verifier
        .verify_token_principal(&token)
        .await
        .expect("owner verifies ID Principal");
    assert_eq!(verified.r#use, of_model::contract::token_use::TokenUse::Id);
    assert_eq!(verified.sub, principal.user.id.to_string());
    assert!(
        verifier
            .validate_access_token(&token, &client.client_id)
            .await
            .is_err(),
        "ID token is not an access token"
    );
    for binding in [None, Some(Id::now_v7())] {
        let mut altered = claims.clone();
        if let Some(binding) = binding {
            altered["principal_key_id"] = binding.to_string().into();
        } else {
            altered
                .as_object_mut()
                .expect("claims object")
                .remove("principal_key_id");
        }
        let invalid = encode_jwt(
            &service.local_signing_jwk().await.expect("Node signer"),
            &altered,
        )
        .expect("sign altered binding");
        assert!(
            verifier.verify_token_principal(&invalid).await.is_err(),
            "missing/wrong signed root must deny"
        );
    }
    service
        .key_service
        .rotate_active_entity_root_key(
            EntityType::User,
            principal.user.id,
            "rotated root".into(),
            None,
        )
        .await
        .expect("rotate local Principal root");
    assert!(
        service.verify_token_principal(&token).await.is_err(),
        "owner learns root rotation"
    );
    assert!(
        verifier.verify_token_principal(&token).await.is_ok(),
        "disconnected verifier has not learned root rotation"
    );
    set(
        &verifier_engine,
        "keys",
        principal.key.id,
        "revoked_at",
        Value::Integer(chrono::Utc::now().timestamp()),
    )
    .await;
    assert!(
        verifier.verify_token_principal(&token).await.is_err(),
        "locally learned root revocation denies ID Principal verification"
    );
}

#[tokio::test]
async fn issuer_rotation_publishes_new_key_without_enabling_verifier_issuance() {
    let (source, authority, client, principal) = setup().await;
    let old = issue(&authority, &client, &principal).await;
    let old_kid = decode_jwt::<StandardClaims>(&old.access_token.0)
        .expect("original claims")
        .0
        .kid;
    let refresh = old.refresh_token.expect("original refresh").0;
    let (_, verifier) = public_verifier(&source, &authority).await;
    assert!(
        verifier.rotate_issuer_key().await.is_err(),
        "a relying Node cannot rotate keys"
    );
    assert_eq!(
        verifier
            .token(grant(&refresh, None), Some(auth(&client)))
            .await
            .expect_err("a relying Node cannot refresh an issuer grant")
            .error,
        of_model::idp::contract::error_code::ErrorCode::AccessDenied
    );

    let replacement = authority
        .rotate_issuer_key()
        .await
        .expect("rotate at the authority");
    assert_ne!(replacement.kid, old_kid);
    let renewed = authority
        .token(grant(&refresh, None), Some(auth(&client)))
        .await
        .expect("old refresh works during verification overlap");
    let new_kid = decode_jwt::<StandardClaims>(&renewed.access_token.0)
        .expect("new claims")
        .0
        .kid;
    assert_eq!(new_kid, replacement.kid);
    let keys = authority.list_jwks().await.expect("current JWKS");
    assert_eq!(keys.keys.len(), 2);
    assert!(keys.keys.contains(&replacement));
    assert!(
        authority
            .validate_access_token(&old.access_token.0, &client.client_id)
            .await
            .is_ok()
    );
    assert!(
        verifier
            .validate_access_token(&old.access_token.0, &client.client_id)
            .await
            .is_ok()
    );
    assert!(
        verifier
            .validate_access_token(&renewed.access_token.0, &client.client_id)
            .await
            .is_err()
    );
    let (_, refreshed_verifier) = public_verifier(&source, &authority).await;
    assert!(
        refreshed_verifier
            .validate_access_token(&renewed.access_token.0, &client.client_id)
            .await
            .is_ok()
    );
}
