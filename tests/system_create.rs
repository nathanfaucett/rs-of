#![cfg(target_os = "linux")]

#[path = "system/create.rs"]
mod create;

#[tokio::test]
#[ignore = "requires built service launcher and identity-database; see tests/system/README.md"]
async fn unified_create_and_use() {
    create::scenario(false).await;
}

#[tokio::test]
#[ignore = "requires built service launcher and identity-database; see tests/system/README.md"]
async fn independent_create_and_use() {
    create::scenario(true).await;
}

#[tokio::test]
#[ignore = "requires built service launcher and identity-database; see tests/system/README.md"]
async fn unified_retained_restart() {
    create::restart_scenario(false).await;
}

#[tokio::test]
#[ignore = "requires built service launcher and identity-database; see tests/system/README.md"]
async fn independent_retained_restart() {
    create::restart_scenario(true).await;
}

#[tokio::test]
#[ignore = "requires built service launcher and identity-database; see tests/system/README.md"]
async fn unified_join_and_source_offline_reopen() {
    create::join_scenario(false).await;
}

#[tokio::test]
#[ignore = "requires built service launcher and identity-database; see tests/system/README.md"]
async fn independent_join_and_source_offline_reopen() {
    create::join_scenario(true).await;
}
