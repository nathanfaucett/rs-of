use std::{io, sync::Arc, time::Duration};

use tokio::sync::{Semaphore, mpsc};
use tokio_util::sync::CancellationToken;

use super::run_peer_refresh;

async fn next_refresh(events: &mut mpsc::UnboundedReceiver<()>) {
    tokio::time::timeout(Duration::from_secs(5), events.recv())
        .await
        .expect("refresh triggered promptly")
        .expect("refresh task alive");
}

async fn databases() -> (std::path::PathBuf, db::Database, db::Database) {
    let root =
        std::env::temp_dir().join(format!("peer-refresh-{}", idp_model::model::Id::now_v7()));
    std::fs::create_dir_all(&root).expect("test directory");
    let idp = db::Database::open(root.join("idp.redb")).expect("IdP database");
    let management = db::Database::open(root.join("devices.redb")).expect("Management database");
    for database in [&idp, &management] {
        database
            .client()
            .execute_sql(
                "CREATE TABLE peers (id UUID PRIMARY KEY, approved BOOLEAN)",
                None,
            )
            .await
            .expect("schema");
    }
    (root, idp, management)
}

async fn commit(database: &db::Database) {
    database
        .client()
        .execute_sql("INSERT INTO peers (approved) VALUES (true)", None)
        .await
        .expect("commit approval");
}

fn changes(idp: &db::Database, management: &db::Database) -> ofdb_sql::ChangeSubscription {
    idp.sync_changes()
        .expect("IdP changes")
        .merge(management.sync_changes().expect("Management changes"))
}

#[tokio::test]
async fn peer_refresh_is_idle_and_coalesces_both_sources() {
    let (root, idp, management) = databases().await;
    let cancellation = CancellationToken::new();
    let (events, mut refreshed) = mpsc::unbounded_channel();
    let gate = Arc::new(Semaphore::new(0));
    let task_gate = gate.clone();
    let task = tokio::spawn(run_peer_refresh(
        changes(&idp, &management),
        cancellation.clone(),
        move || {
            let gate = task_gate.clone();
            let events = events.clone();
            async move {
                events.send(()).expect("record refresh");
                gate.acquire().await.expect("release refresh").forget();
                Ok(())
            }
        },
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(2100), refreshed.recv())
            .await
            .is_err()
    );
    commit(&idp).await;
    next_refresh(&mut refreshed).await;
    for _ in 1..=10 {
        commit(&idp).await;
        commit(&management).await;
    }
    gate.add_permits(1);
    next_refresh(&mut refreshed).await;
    gate.add_permits(1);
    assert!(
        tokio::time::timeout(Duration::from_millis(100), refreshed.recv())
            .await
            .is_err()
    );
    commit(&management).await;
    next_refresh(&mut refreshed).await;
    // Cancellation must also interrupt an in-flight refresh.
    cancellation.cancel();
    tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .expect("cancel promptly")
        .expect("refresh task");
    drop((idp, management));
    std::fs::remove_dir_all(root).expect("remove test databases");
}

#[tokio::test]
async fn peer_refresh_retries_failures_without_another_commit() {
    let (root, idp, management) = databases().await;
    let cancellation = CancellationToken::new();
    let (events, mut refreshed) = mpsc::unbounded_channel();
    let mut fail = true;
    let task = tokio::spawn(run_peer_refresh(
        changes(&idp, &management),
        cancellation.clone(),
        move || {
            events.send(()).expect("record refresh");
            let result = if std::mem::take(&mut fail) {
                Err(io::Error::other("injected failure"))
            } else {
                Ok(())
            };
            std::future::ready(result)
        },
    ));
    commit(&idp).await;
    next_refresh(&mut refreshed).await;
    next_refresh(&mut refreshed).await;
    assert!(
        tokio::time::timeout(Duration::from_millis(2100), refreshed.recv())
            .await
            .is_err()
    );
    cancellation.cancel();
    task.await.expect("refresh task");
    drop((idp, management));
    std::fs::remove_dir_all(root).expect("remove test databases");
}

#[tokio::test]
async fn peer_refresh_tracks_real_sql_commits_and_read_commits_are_silent() {
    let (root, idp, management) = databases().await;
    let changes = changes(&idp, &management);
    // A commit between subscription and task startup must not be lost.
    idp.client()
        .execute_sql("INSERT INTO peers (approved) VALUES (true)", None)
        .await
        .expect("initial approval");
    let cancellation = CancellationToken::new();
    let (events, mut refreshed) = mpsc::unbounded_channel();
    let clients = [idp.client(), management.client()];
    let task = tokio::spawn(run_peer_refresh(changes, cancellation.clone(), move || {
        let clients = clients.clone();
        let events = events.clone();
        async move {
            for client in clients {
                client
                    .execute_sql("SELECT * FROM peers", None)
                    .await
                    .map_err(io::Error::other)?;
            }
            events.send(()).expect("record refresh");
            Ok(())
        }
    }));
    next_refresh(&mut refreshed).await;
    management
        .client()
        .execute_sql("INSERT INTO peers (approved) VALUES (true)", None)
        .await
        .expect("Management approval");
    next_refresh(&mut refreshed).await;
    idp.client()
        .execute_sql("UPDATE peers SET approved = false", None)
        .await
        .expect("IdP revocation");
    next_refresh(&mut refreshed).await;
    assert!(
        tokio::time::timeout(Duration::from_millis(2100), refreshed.recv())
            .await
            .is_err(),
        "read commits must not self-trigger or poll"
    );
    cancellation.cancel();
    task.await.expect("refresh task");
    drop((idp, management));
    std::fs::remove_dir_all(root).expect("remove test databases");
}
