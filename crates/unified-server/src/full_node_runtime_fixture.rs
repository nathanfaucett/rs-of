use std::sync::Arc;

use db::Database;
use iroh_chain::Server;

use super::UnifiedRuntime;

impl UnifiedRuntime {
    pub(crate) fn full_node_test_management(&self) -> Arc<Database> {
        Arc::clone(&self.device_engine)
    }

    pub(crate) fn full_node_test_server(&self) -> &Server {
        self.endpoint.server()
    }
}
