use std::{
    collections::BTreeSet,
    sync::{Arc, RwLock},
};

use iroh::EndpointId;
use tokio::sync::watch;

#[derive(Clone, Debug)]
pub struct EndpointIdStore {
    ids: Arc<RwLock<BTreeSet<EndpointId>>>,
    changes: watch::Sender<u64>,
}

impl Default for EndpointIdStore {
    fn default() -> Self {
        Self {
            ids: Arc::default(),
            changes: watch::channel(0).0,
        }
    }
}

impl EndpointIdStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ids(&self) -> Vec<EndpointId> {
        self.ids
            .read()
            .expect("endpoint store poisoned")
            .iter()
            .copied()
            .collect()
    }

    pub fn contains(&self, id: EndpointId) -> bool {
        self.ids
            .read()
            .expect("endpoint store poisoned")
            .contains(&id)
    }

    /// Observes coalesced inventory changes, not connection state.
    /// Subscribe before reading the inventory to avoid missing concurrent changes.
    pub fn subscribe_changes(&self) -> watch::Receiver<u64> {
        self.changes.subscribe()
    }

    pub fn replace(&self, ids: impl IntoIterator<Item = EndpointId>) {
        let replacement = ids.into_iter().collect();
        {
            let mut ids = self.ids.write().expect("endpoint store poisoned");
            if *ids == replacement {
                return;
            }
            *ids = replacement;
        }
        self.notify_changed();
    }

    pub fn add(&self, id: EndpointId) -> bool {
        let changed = self
            .ids
            .write()
            .expect("endpoint store poisoned")
            .insert(id);
        if changed {
            self.notify_changed();
        }
        changed
    }

    pub fn remove(&self, id: EndpointId) -> bool {
        let changed = self
            .ids
            .write()
            .expect("endpoint store poisoned")
            .remove(&id);
        if changed {
            self.notify_changed();
        }
        changed
    }

    fn notify_changed(&self) {
        self.changes.send_modify(|generation| {
            *generation = generation.wrapping_add(1);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::EndpointIdStore;
    use iroh::SecretKey;

    #[test]
    fn membership_changes_notify() {
        let peer = SecretKey::generate().public();
        let store = EndpointIdStore::new();
        let mut changes = store.subscribe_changes();
        assert_eq!(*changes.borrow_and_update(), 0);
        assert!(store.add(peer));
        assert!(changes.has_changed().expect("store sender is alive"));
        assert_eq!(*changes.borrow_and_update(), 1);
        assert_eq!(store.ids(), vec![peer]);
        assert!(store.remove(peer));
        assert!(changes.has_changed().expect("store sender is alive"));
        assert_eq!(*changes.borrow_and_update(), 2);
        store.replace([peer]);
        assert!(changes.has_changed().expect("store sender is alive"));
        assert_eq!(*changes.borrow_and_update(), 3);
        store.replace([]);
        assert_eq!(*changes.borrow_and_update(), 4);
        assert!(store.ids().is_empty());
    }

    #[test]
    fn identical_membership_does_not_notify() {
        let first = SecretKey::generate().public();
        let second = SecretKey::generate().public();
        let store = EndpointIdStore::new();
        let mut changes = store.subscribe_changes();
        store.replace([]);
        assert!(!store.remove(first));
        assert!(!changes.has_changed().expect("store sender is alive"));
        store.replace([first, second]);
        assert_eq!(*changes.borrow_and_update(), 1);
        store.replace([second, first, first]);
        assert!(!store.add(first));
        assert!(!store.remove(SecretKey::generate().public()));
        assert!(!changes.has_changed().expect("store sender is alive"));
        assert_eq!(*changes.borrow(), 1);
    }

    #[tokio::test]
    async fn cloned_stores_share_notifications_and_retain_generation() {
        let first = SecretKey::generate().public();
        let second = SecretKey::generate().public();
        let store = EndpointIdStore::default();
        assert!(store.add(first));
        let shared = store.clone();
        let mut changes = store.subscribe_changes();
        let mut shared_changes = shared.subscribe_changes();
        assert_eq!(*changes.borrow(), 1);
        assert!(!changes.has_changed().expect("store sender is alive"));
        assert!(shared.add(second));
        changes.changed().await.expect("store sender is alive");
        shared_changes
            .changed()
            .await
            .expect("store sender is alive");
        assert_eq!(*changes.borrow(), 2);
        assert_eq!(*shared_changes.borrow(), 2);
        assert!(store.contains(second));
        drop(store);
        assert!(shared.remove(first));
        changes.changed().await.expect("clone keeps sender alive");
        assert_eq!(*changes.borrow(), 3);
    }

    #[test]
    fn shared_store_updates() {
        let first = SecretKey::generate().public();
        let second = SecretKey::generate().public();
        let store = EndpointIdStore::new();
        let shared = store.clone();
        assert!(store.add(first));
        assert!(shared.contains(first));
        shared.replace([second]);
        assert!(!store.contains(first));
        assert_eq!(store.ids(), vec![second]);
        assert!(store.remove(second));
        assert!(shared.ids().is_empty());
    }
}
