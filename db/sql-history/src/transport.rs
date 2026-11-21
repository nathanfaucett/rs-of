use alloc::vec::Vec;

/// Ordered byte frames. Implementations must bound reads and queued frames
/// before buffering; session size checks happen after transport receipt.
pub trait SyncTransport {
    type Error;

    fn receive(
        &mut self,
    ) -> impl Future<Output = Result<Vec<u8>, Self::Error>> + platform::MaybeSend;
    fn send(
        &mut self,
        frame: Vec<u8>,
    ) -> impl Future<Output = Result<(), Self::Error>> + platform::MaybeSend;
}
