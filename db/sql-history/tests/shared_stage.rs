#![cfg(feature = "shared-stage")]

use std::{env, fs};

use btree_redb::SharedDatabase;
use futures::executor::block_on;
use history::{NativeSyncStageFactory, SyncStage, SyncStageFactory};

#[test]
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
        drop(stage);
        let mut stages = Vec::new();
        for _ in 0..128 {
            stages.push(factory.create().await.expect("available shared stage slot"));
        }
        assert!(factory.create().await.is_err());
        drop(stages.pop());
        assert!(factory.create().await.is_ok());
        drop(stages);
        drop(factory);
        drop(host);
        fs::remove_file(path).expect("remove single host file");
    });
}
