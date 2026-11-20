use bytes::Bytes;
use ofdb_platform::{BoxFuture, MaybeSend, MaybeSync};
use uuid::Uuid;

use crate::Error;

/// Object-safe content boundary shared across threads on native targets.
pub trait ContentObject: ContentStore + MaybeSend + MaybeSync {}
impl<T: ?Sized + ContentStore + MaybeSend + MaybeSync> ContentObject for T {}

/// Object-safe staged revision boundary shared across threads on native targets.
pub trait StagedObject: Staged + MaybeSend {}
impl<T: ?Sized + Staged + MaybeSend> StagedObject for T {}

/// Internal port for complete Content Revisions addressed by File id and Content Revision id.
///
/// The boundary is internal to the library: it is not a public extension point.
pub trait ContentStore: MaybeSend + MaybeSync {
    /// Whether the revision's bytes are stored.
    fn has(&self, file_id: Uuid, revision: Uuid) -> BoxFuture<'_, Result<bool, Error>>;

    /// Returns the retained revision length without fetching remote content.
    fn content_len(&self, file_id: Uuid, revision: Uuid) -> BoxFuture<'_, Result<u64, Error>>;

    /// Reads a complete revision.
    fn read(&self, file_id: Uuid, revision: Uuid) -> BoxFuture<'_, Result<Vec<u8>, Error>>;

    /// Reads up to `length` bytes of a revision starting at `offset`.
    /// An empty result ends the revision.
    fn read_range(
        &self,
        file_id: Uuid,
        revision: Uuid,
        offset: u64,
        length: u32,
    ) -> BoxFuture<'_, Result<Bytes, Error>>;

    /// Begins a new staged revision. Dropping an unpublished stage removes it.
    fn stage(
        &self,
        file_id: Uuid,
        revision: Uuid,
    ) -> BoxFuture<'_, Result<Box<dyn StagedObject>, Error>>;

    /// Removes every stored revision of a file.
    fn remove(&self, file_id: Uuid) -> BoxFuture<'_, Result<(), Error>>;
}

/// A staged revision write. Publishing makes the bytes durable at the revision's address;
/// dropping an unpublished stage discards it.
pub trait Staged: MaybeSend {
    fn write<'a>(&'a mut self, bytes: &'a [u8]) -> BoxFuture<'a, Result<(), Error>>;

    /// Copies a previous revision's bytes as the staged revision's starting content.
    fn copy_from(&mut self, file_id: Uuid, revision: Uuid) -> BoxFuture<'_, Result<(), Error>>;

    fn write_at<'a>(&'a mut self, offset: u64, bytes: &'a [u8])
    -> BoxFuture<'a, Result<(), Error>>;

    fn write_end<'a>(&'a mut self, bytes: &'a [u8]) -> BoxFuture<'a, Result<(), Error>>;

    fn truncate(&mut self, length: u64) -> BoxFuture<'_, Result<(), Error>>;

    fn sync(&mut self) -> BoxFuture<'_, Result<(), Error>>;

    fn publish(&mut self) -> BoxFuture<'_, Result<(), Error>>;

    /// Marks the published bytes as committed so dropping no longer removes them.
    fn commit(&mut self);
}
