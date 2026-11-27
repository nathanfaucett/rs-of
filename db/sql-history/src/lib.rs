#![no_std]

extern crate alloc;
#[cfg(all(
    feature = "native-stage",
    not(all(target_arch = "wasm32", target_os = "unknown"))
))]
extern crate std;

mod candidate;
mod codec;
#[cfg(all(
    feature = "native-stage",
    not(all(target_arch = "wasm32", target_os = "unknown"))
))]
mod file_stage;
#[cfg(all(
    feature = "native-stage",
    not(all(target_arch = "wasm32", target_os = "unknown"))
))]
pub use file_stage::{FileSyncStage, FileSyncStageFactory};
mod pre_apply;
mod protocol;
mod scope;
mod selection;
pub use selection::SyncSelection;
mod session;
mod staging;
mod state;
mod transport;

pub use candidate::{SyncCandidateView, SyncRowView, SyncTableView, SyncValidationDirection};
pub use codec::{InspectedHistory, SyncRowCodec};
pub use protocol::{
    PROTOCOL_VERSION, SyncHello, SyncIncrementalChange, SyncMessage, SyncRowInventory,
    SyncSnapshotRequest,
};
pub use session::{
    DEFAULT_MAX_SESSION_BYTES, MAX_APPLY_BATCH_BYTES, MAX_MESSAGE_BYTES, SessionConfig, SyncError,
    SyncResult, SyncRole, apply_incremental_changes_for, apply_sync_state_batch_for,
    apply_sync_state_for, catalog_history_is_empty_for, export_sync_state_bounded_for,
    export_sync_state_for, initial_restore_tag_for, publish_initial_history_for,
    replace_initial_restore_tag_for, restore_initial_history_for, sync_manifest_for, synchronize,
    synchronize_pull_with_stage_factory, synchronize_selected_with_stage_factory_checked,
    synchronize_tables_with_stage_factory_candidate_validated_checked,
    synchronize_tables_with_stage_factory_checked,
    synchronize_tables_with_stage_factory_validated_checked, synchronize_with_stage_factory,
    synchronize_with_stage_factory_checked,
};
pub use staging::{MemorySyncStageFactory, SyncStage, SyncStageFactory};
pub use state::{StateDigest, SyncChangeId, SyncKey, SyncManifest, SyncStateUnit};
pub use transport::SyncTransport;

#[cfg(feature = "shared-stage")]
mod shared_stage;
#[cfg(feature = "shared-stage")]
pub use shared_stage::{NativeSyncStage, NativeSyncStageFactory};
