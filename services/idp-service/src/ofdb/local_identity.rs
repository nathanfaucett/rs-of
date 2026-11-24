use core::fmt;
use std::{fmt::Display, io};

use of_model::id::Id;
use ofdb_sql::Client;

use crate::ofdb::user_repo::DbUserRepo;
use crate::{password_config::PasswordConfig, repo::user_repo::UserRepo};

#[derive(Clone)]
pub struct LocalIdentity {
    engine: Client,
}

impl fmt::Debug for LocalIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalIdentity").finish_non_exhaustive()
    }
}

impl LocalIdentity {
    #[must_use]
    pub const fn new(engine: Client) -> Self {
        Self { engine }
    }

    /// # Errors
    /// Returns an I/O error if the local identity lookup fails.
    pub async fn is_active_user(&self, subject: Id) -> io::Result<bool> {
        if subject.is_nil() {
            return Ok(false);
        }
        Ok(
            DbUserRepo::new(self.engine.clone(), PasswordConfig::default())
                .find_user_by_id(subject)
                .await
                .map_err(denied)?
                .is_some_and(|user| !user.blocked),
        )
    }
}

fn denied(reason: impl Display) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, reason.to_string())
}
