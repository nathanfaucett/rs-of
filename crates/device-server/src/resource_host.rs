use std::{io, sync::{Arc, RwLock}};

use iroh::{endpoint::Connection, protocol::{AcceptError, DynProtocolHandler, ProtocolHandler}};
use iroh_chain::Server;

#[derive(Clone)]
pub struct DeviceResourceHost {
    server: Server,
    handler: ResourceHandler,
}
impl DeviceResourceHost {
    pub(crate) fn new(server: Server) -> Self { Self { server, handler: ResourceHandler::default() } }
    pub fn server(&self) -> Server { self.server.clone() }
    pub fn install<D: ProtocolHandler>(&self, handler: D) -> io::Result<()> {
        let mut current = self.handler.0.write().map_err(|_| io::Error::other("DATA owner lock poisoned"))?;
        if current.is_some() { return Err(io::Error::other("Resource DATA owner is already installed")); }
        *current = Some(Arc::new(handler));
        Ok(())
    }
    pub(crate) fn handler(&self) -> impl ProtocolHandler { self.handler.clone() }
    pub(crate) fn close(&self) {
        if let Ok(mut handler) = self.handler.0.write() { *handler = None; }
    }
}

#[derive(Clone, Default, Debug)]
struct ResourceHandler(Arc<RwLock<Option<Arc<dyn DynProtocolHandler>>>>);
impl ProtocolHandler for ResourceHandler {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        let handler = self.0.read().ok().and_then(|handler| handler.clone());
        if let Some(handler) = handler { handler.accept(connection).await }
        else { connection.close(1u32.into(), b"Resource DATA owner unavailable"); Ok(()) }
    }
}
