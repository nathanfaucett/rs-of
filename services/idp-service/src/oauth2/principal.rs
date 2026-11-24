use core::{any::Any, fmt};

use of_model::idp::contract::entity_type::EntityType;

use of_model::idp::model::client::Client;

use of_model::id::Id;

use of_model::idp::model::key::Key;

use of_model::idp::model::user::User;

pub trait Principal: Send + Sync {
    fn get_entity_id(&self) -> Id;
    fn get_entity_type(&self) -> EntityType;
    fn get_entity_as_any(&self) -> &dyn Any;
    fn get_key(&self) -> &Key;
}

pub struct UserPrincipal {
    pub user: User,
    pub key: Key,
}

pub struct ClientPrincipal {
    pub client: Client,
    pub key: Key,
}

impl fmt::Debug for UserPrincipal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserPrincipal").finish_non_exhaustive()
    }
}

impl fmt::Debug for ClientPrincipal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientPrincipal").finish_non_exhaustive()
    }
}

impl Principal for UserPrincipal {
    fn get_entity_id(&self) -> Id {
        self.user.id
    }

    fn get_entity_type(&self) -> EntityType {
        EntityType::User
    }

    fn get_entity_as_any(&self) -> &dyn Any {
        &self.user
    }

    fn get_key(&self) -> &Key {
        &self.key
    }
}

impl Principal for ClientPrincipal {
    fn get_entity_id(&self) -> Id {
        self.client.id
    }

    fn get_entity_type(&self) -> EntityType {
        EntityType::Client
    }

    fn get_entity_as_any(&self) -> &dyn Any {
        &self.client
    }

    fn get_key(&self) -> &Key {
        &self.key
    }
}
