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
