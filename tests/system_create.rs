#![cfg(target_os = "linux")]

#[path = "system/create.rs"]
mod create;

#[tokio::test]
#[ignore = "requires built service launcher and identity-database; see tests/system/README.md"]
async fn independent_create_and_use() {
    create::scenario().await;
}

#[tokio::test]
#[ignore = "requires built service launcher and identity-database; see tests/system/README.md"]
async fn independent_retained_restart() {
    create::restart_scenario().await;
}

#[tokio::test]
#[ignore = "requires built service launcher and identity-database; see tests/system/README.md"]
async fn independent_join_and_source_offline_reopen() {
    create::join_scenario().await;
}

#[tokio::test]
#[ignore = "requires built service launcher; see tests/system/README.md"]
async fn independent_local_owner_create_and_restart() {
    create::local_restart_scenario().await;
}

#[tokio::test]
#[ignore = "requires built service launcher; see tests/system/README.md"]
async fn independent_permission_resource_denial() {
    create::denial_scenario().await;
}
