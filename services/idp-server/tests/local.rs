use std::{env, fs, sync::Arc};

use db::adapters::MemoryDatabase;
use idp_service::{
    ofdb::user_repo::DbUserRepo, password_config::PasswordConfig, repo::user_repo::UserRepo,
};
use of_model::id::Id;

#[tokio::test]
async fn local_constructor_opens_its_owner_file_without_setup() {
    let root = env::temp_dir().join(format!("idp-owner-local-{}", Id::now_v7()));
    let router = idp_server::build_local_router(
        &root,
        "http://127.0.0.1:1355/idp",
        "http://localhost:5173",
        None,
        None,
        None,
        &[],
    )
    .await
    .expect("local IdP owner");
    assert!(root.join("idp.redb").is_file());
    assert!(!root.join("unified.redb").exists());
    assert!(!root.join("idp.setup-host").exists());
    drop(router);
    let router = idp_server::build_local_router(
        &root,
        "http://127.0.0.1:1355/idp",
        "http://localhost:5173",
        None,
        None,
        None,
        &[],
    )
    .await
    .expect("reopen local IdP owner");
    drop(router);
    fs::remove_dir_all(root).expect("remove local IdP owner");
}

#[tokio::test]
async fn local_constructor_seeds_frontends_and_admin_without_setup_and_is_repeatable() {
    let root = env::temp_dir().join(format!("idp-local-{}", Id::now_v7()));
    let adapter = Arc::new(MemoryDatabase::new().await.expect("shared schema"));
    let db = Arc::new(adapter.database());
    for _ in 0..2 {
        let router = idp_server::build_local_router(
            &root,
            "http://127.0.0.1:1355/idp",
            "http://localhost:5173",
            None,
            Some(Arc::clone(&db)),
            Some(adapter.storage()),
            &[],
        )
        .await
        .expect("local router without setup");
        drop(router);
    }
    let users = DbUserRepo::new(adapter.client(), PasswordConfig::default());
    let admin = users
        .find_user_by_username_or_email("admin@local.test")
        .await
        .expect("admin lookup")
        .expect("seeded admin");
    let password = users
        .find_user_password_by_user_id(admin.id)
        .await
        .expect("password lookup")
        .expect("password credential");
    assert!(
        argon2::verify_encoded(&password.password_hash, b"password")
            .expect("verify seeded password")
    );
    let clients = adapter
        .client()
        .execute_sql(
            "SELECT client_id, redirect_uris FROM clients ORDER BY client_id",
            None,
        )
        .await
        .expect("seeded clients");
    let text = format!("{:?}", clients[0].rows);
    assert!(text.contains("password-manager"));
    assert!(text.contains("http://localhost:5174/callback"));
    assert!(text.contains("http://localhost:5173/callback"));
    assert!(!root.join("idp.redb").exists());
    assert!(!root.join("idp.setup-host").exists());
    if root.exists() {
        fs::remove_dir_all(root).expect("remove local fixture");
    }
}

#[tokio::test]
async fn local_constructor_accepts_a_password_manager_origin() {
    let root = env::temp_dir().join(format!("idp-local-origins-{}", Id::now_v7()));
    let adapter = Arc::new(MemoryDatabase::new().await.expect("shared schema"));
    let db = Arc::new(adapter.database());
    let router = idp_server::build_local_router(
        &root,
        "http://127.0.0.1:1355/idp",
        "http://localhost:4173",
        Some("http://localhost:4174"),
        Some(db),
        Some(adapter.storage()),
        &[],
    )
    .await
    .expect("local router with separate frontend origins");
    drop(router);
    let clients = adapter
        .client()
        .execute_sql("SELECT redirect_uris FROM clients", None)
        .await
        .expect("seeded clients");
    let text = format!("{:?}", clients[0].rows);
    assert!(text.contains("http://localhost:4173/callback"));
    assert!(text.contains("http://localhost:4174/callback"));
    assert!(!text.contains("http://localhost:5174/callback"));
    fs::remove_dir_all(root).expect("remove local fixture");
}
