use futures::{FutureExt, StreamExt};

use crate::changes::ChangeSignal;

#[tokio::test]
async fn notifications_are_coalesced_and_cancel_safe() {
    let signal = ChangeSignal::default();
    let mut changes = signal.subscribe();
    signal.committed();
    signal.committed();
    // Events are available immediately after commit
    let mut stream = changes.events();
    assert!(
        stream.next().now_or_never().is_some(),
        "notification received"
    );
    signal.committed();
    let mut clone = changes.clone();
    assert!(
        clone.events().next().now_or_never().is_some(),
        "cloned notification received"
    );
    assert!(
        changes.events().next().now_or_never().is_some(),
        "original notification received"
    );
    signal.committed();
    changes.consume();
    // After consume, no more historical events
    assert!(
        changes.events().next().now_or_never().is_none(),
        "no more events after consume"
    );
}

#[tokio::test]
async fn merged_notifications_close_only_after_all_senders_close() {
    let a = ChangeSignal::default();
    let b = ChangeSignal::default();
    let mut changes = a.subscribe().merge(b.subscribe());
    a.committed();
    b.committed();
    // Events available immediately
    assert!(
        changes.events().next().now_or_never().is_some(),
        "merged notification received"
    );
    drop(a);
    // a dropped, but b still alive - stream should not close yet
    assert!(changes.closed().now_or_never().is_none());
    b.committed();
    drop(b);
    // Both senders dropped - wait for closed
    changes.closed().await;
    // After closed, no more events
    assert!(changes.events().next().await.is_none());
}

#[cfg(test)]
mod change_event_tests {
    use crate::changes::{ChangeEvent, ChangeSignal, StorageKey};
    use futures::StreamExt;

    #[tokio::test]
    async fn committed_with_events_single_event() {
        let signal = ChangeSignal::default();
        let mut changes = signal.subscribe();
        let event = ChangeEvent {
            key: StorageKey {
                table: "test".into(),
                key: b"key1".to_vec(),
            },
            blob: vec![1, 2, 3],
            generation: 1,
        };
        signal.committed_with_events([event.clone()]);
        let mut stream = changes.events();
        let e = stream.next().await.unwrap();
        assert_eq!(e.key.table, "test");
        assert_eq!(e.key.key, b"key1");
        assert_eq!(e.blob, vec![1, 2, 3]);
        assert_eq!(e.generation, 1);
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn committed_with_events_multiple_events() {
        let signal = ChangeSignal::default();
        let mut changes = signal.subscribe();
        let events = vec![
            ChangeEvent {
                key: StorageKey {
                    table: "test".into(),
                    key: b"key1".to_vec(),
                },
                blob: vec![1, 2, 3],
                generation: 1,
            },
            ChangeEvent {
                key: StorageKey {
                    table: "test".into(),
                    key: b"key2".to_vec(),
                },
                blob: vec![4, 5, 6],
                generation: 2,
            },
        ];
        signal.committed_with_events(events.clone());
        let mut stream = changes.events();
        let e1 = stream.next().await.unwrap();
        let e2 = stream.next().await.unwrap();
        assert_eq!(e1.generation, 1);
        assert_eq!(e2.generation, 2);
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn committed_with_events_large_blob_fallback() {
        let signal = ChangeSignal::default();
        let mut changes = signal.subscribe();
        let large_blob = vec![0u8; 1024 * 1024]; // 1MB
        let event = ChangeEvent {
            key: StorageKey {
                table: "test".into(),
                key: b"key1".to_vec(),
            },
            blob: large_blob.clone(),
            generation: 1,
        };
        signal.committed_with_events([event]);
        let mut stream = changes.events();
        let e = stream.next().await.unwrap();
        assert_eq!(e.blob.len(), 1024 * 1024);
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn events_stream_yields_multiple_events_in_order() {
        let signal = ChangeSignal::default();
        let mut changes = signal.subscribe();
        let events = vec![
            ChangeEvent {
                key: StorageKey {
                    table: "test".into(),
                    key: b"key1".to_vec(),
                },
                blob: vec![1],
                generation: 2,
            },
            ChangeEvent {
                key: StorageKey {
                    table: "test".into(),
                    key: b"key2".to_vec(),
                },
                blob: vec![2],
                generation: 1,
            },
            ChangeEvent {
                key: StorageKey {
                    table: "test".into(),
                    key: b"key3".to_vec(),
                },
                blob: vec![3],
                generation: 3,
            },
        ];
        signal.committed_with_events(events);
        let mut stream = changes.events();
        let e1 = stream.next().await.unwrap();
        let e2 = stream.next().await.unwrap();
        let e3 = stream.next().await.unwrap();
        assert_eq!(e1.generation, 1);
        assert_eq!(e2.generation, 2);
        assert_eq!(e3.generation, 3);
        assert!(stream.next().await.is_none());
    }
}

#[cfg(test)]
mod change_event_tests2 {
    use crate::changes::{ChangeEvent, ChangeSignal, StorageKey};
    use futures::StreamExt;

    #[tokio::test]
    async fn committed_with_events_large_blob_fallback() {
        let signal = ChangeSignal::default();
        let mut changes = signal.subscribe();
        let large_blob = vec![0u8; 1024 * 1024]; // 1MB
        let event = ChangeEvent {
            key: StorageKey {
                table: "test".into(),
                key: b"key1".to_vec(),
            },
            blob: large_blob.clone(),
            generation: 1,
        };
        signal.committed_with_events([event]);
        let mut stream = changes.events();
        let e = stream.next().await.unwrap();
        assert_eq!(e.blob.len(), 1024 * 1024);
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn events_stream_yields_multiple_events_in_order() {
        let signal = ChangeSignal::default();
        let mut changes = signal.subscribe();
        let events = vec![
            ChangeEvent {
                key: StorageKey {
                    table: "test".into(),
                    key: b"key1".to_vec(),
                },
                blob: vec![1],
                generation: 2,
            },
            ChangeEvent {
                key: StorageKey {
                    table: "test".into(),
                    key: b"key2".to_vec(),
                },
                blob: vec![2],
                generation: 1,
            },
            ChangeEvent {
                key: StorageKey {
                    table: "test".into(),
                    key: b"key3".to_vec(),
                },
                blob: vec![3],
                generation: 3,
            },
        ];
        signal.committed_with_events(events);
        let mut stream = changes.events();
        let e1 = stream.next().await.unwrap();
        let e2 = stream.next().await.unwrap();
        let e3 = stream.next().await.unwrap();
        assert_eq!(e1.generation, 1);
        assert_eq!(e2.generation, 2);
        assert_eq!(e3.generation, 3);
        assert!(stream.next().await.is_none());
    }
}
