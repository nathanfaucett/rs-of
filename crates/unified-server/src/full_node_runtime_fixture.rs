use std::sync::Arc;

use db::Database;
use iroh_chain::Server;

use super::UnifiedRuntime;

impl UnifiedRuntime {
    pub(crate) fn full_node_test_identity(&self) -> Arc<Database> {
        self.owner
            .node_join_database()
            .expect("local full-node identity owner")
    }

    pub(crate) fn full_node_test_devices(&self) -> Arc<Database> {
        Arc::clone(&self.device_engine)
    }

    pub(crate) fn full_node_test_server(&self) -> &Server {
        self.endpoint.server()
    }
}
