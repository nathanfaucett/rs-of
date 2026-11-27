#![cfg(feature = "shared-stage")]

use std::{
    env, fs, thread,
    time::{Duration, Instant},
};

use btree_redb::SharedDatabase;
use futures::executor::block_on;
use history::{NativeSyncStageFactory, SyncStage, SyncStageFactory};

#[test]
#[allow(
    clippy::significant_drop_tightening,
    reason = "commit consumes and releases the writer"
)]
fn shared_stages_store_ordered_records_in_the_host_only() {
    block_on(async {
        let path = env::temp_dir().join(format!("shared-stage-{}", uuid::Uuid::now_v7()));
        let host = SharedDatabase::create(&path).expect("host");
        let factory =
            NativeSyncStageFactory::shared(host.namespace("sql:resource").expect("namespace"));
        let mut stage = factory.create().await.expect("stage");
        stage.append(b"first".to_vec()).await.expect("first record");
        stage
            .append(b"second".to_vec())
            .await
            .expect("second record");
        stage.finish().await.expect("finish");
        assert!(stage.append(vec![]).await.is_err());
        assert_eq!(
            stage.next_record().await.expect("first"),
            Some(b"first".to_vec())
        );
        assert_eq!(
            stage.next_record().await.expect("second"),
            Some(b"second".to_vec())
        );
        assert_eq!(stage.next_record().await.expect("end"), None);
        let mut stages = Vec::new();
        for _ in 1..128 {
            stages.push(factory.create().await.expect("available shared stage slot"));
        }
        factory.create().await.expect_err("the stage limit is full");
        let writer = host
            .namespace("writer")
            .expect("writer namespace")
            .database();
        let transaction = writer.transaction().await.expect("writer transaction");
        transaction
            .create_table::<u64, u64>("write")
            .expect("force physical writer");
        transaction
            .commit_checked(|| async {
                drop(stage);
                factory
                    .create()
                    .await
                    .expect_err("queued cleanup keeps its slot");
                Ok::<(), ()>(())
            })
            .await
            .expect("commit writer")
            .expect("drop did not wait for writer");
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if factory.create().await.is_ok() {
                break;
            }
            assert!(Instant::now() < deadline, "cleanup released its slot");
            thread::sleep(Duration::from_millis(10));
        }
        drop(stages);
        drop(factory);
        drop(host);
        fs::remove_file(path).expect("remove single host file");
    });
}
