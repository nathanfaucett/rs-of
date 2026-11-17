use std::io;
use std::path::Path;

use crate::config::app_config::AppConfig;

/// Service client provisioning - TODO: implement with updated idp-service API
pub async fn run(
    _app_config: &AppConfig,
    _application_uri: &str,
    _client_name: &str,
    _audiences: Vec<String>,
    _scopes: Vec<String>,
    _credentials_path: &Path,
) -> io::Result<()> {
    // TODO: Implement service client provisioning with updated idp-service API
    // Currently disabled due to API mismatches after monorepo merge
    Ok(())
}

fn validate_values(
    application_uri: &str,
    client_name: &str,
    audiences: &[String],
    scopes: &[String],
) -> io::Result<()> {
    if application_uri.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "application URI is required",
        ));
    }
    if client_name.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "client name is required",
        ));
    }
    if audiences.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "at least one audience is required",
        ));
    }
    if scopes.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "at least one scope is required",
        ));
    }
    Ok(())
}
