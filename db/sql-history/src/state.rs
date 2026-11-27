use alloc::{string::String, vec::Vec};

use engine::Uuid;
use serde::{Deserialize, Serialize};

pub use engine::{DocumentChangeId as SyncChangeId, DocumentDigest as StateDigest};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum SyncKey {
    Row { table: String, row: Uuid },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SyncStateUnit {
    pub key: SyncKey,
    pub state: Vec<u8>,
    pub metadata: Vec<u8>,
    pub digest: StateDigest,
}

impl SyncStateUnit {
    #[must_use]
    pub fn new(key: SyncKey, state: Vec<u8>, metadata: Vec<u8>) -> Self {
        let digest = Self::digest_parts(&state, &metadata);
        Self {
            key,
            state,
            metadata,
            digest,
        }
    }

    #[must_use]
    pub fn verify_digest(&self) -> bool {
        Self::digest_parts(&self.state, &self.metadata) == self.digest
    }

    #[must_use]
    pub fn digest_parts(state: &[u8], metadata: &[u8]) -> StateDigest {
        StateDigest::from_parts(state, metadata)
    }
}

/// SHA-256 revision of the ordered catalog and row history, not of the visible SQL rows.
pub type HistoryRevision = [u8; 32];

/// Identifies an exact publication within one destination database.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HistoryPublication {
    pub operation: Uuid,
    pub expected: HistoryRevision,
    pub candidate: HistoryRevision,
}

/// Computes a revision for history units in canonical key order.
#[must_use]
pub fn history_revision(units: &[SyncStateUnit]) -> HistoryRevision {
    use sha2::{Digest, Sha256};

    let mut hash = Sha256::new();
    hash.update(b"ofdb-sql-history-revision-v1");
    for unit in units {
        let SyncKey::Row { table, row } = &unit.key;
        hash.update((table.len() as u64).to_le_bytes());
        hash.update(table.as_bytes());
        hash.update(row.as_bytes());
        hash.update(unit.digest.0);
    }
    hash.finalize().into()
}

#[cfg(test)]
mod tests {
    use alloc::{string::String, vec};

    use super::{SyncKey, SyncStateUnit};
    use engine::Uuid;

    #[test]
    fn digest_verification_does_not_require_mutating_state() {
        let mut unit = SyncStateUnit::new(
            SyncKey::Row {
                table: String::from("items"),
                row: Uuid::from_bytes([1; 16]),
            },
            vec![7; 1024],
            vec![9; 256],
        );

        assert!(unit.verify_digest());
        unit.state[0] ^= 1;
        assert!(!unit.verify_digest());
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SyncManifest {
    pub entries: Vec<(SyncKey, StateDigest)>,
}

impl SyncManifest {
    #[must_use]
    pub fn new(mut entries: Vec<(SyncKey, StateDigest)>) -> Self {
        entries.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        entries.dedup_by(|left, right| left.0 == right.0);
        Self { entries }
    }

    #[must_use]
    pub fn contains(&self, key: &SyncKey, digest: StateDigest) -> bool {
        self.entries
            .binary_search_by(|(candidate, _)| candidate.cmp(key))
            .is_ok_and(|index| self.entries[index].1 == digest)
    }
}
