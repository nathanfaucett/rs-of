use std::sync::Arc;

use idp_service::{
    oauth2::OAuth2Service,
    replica::{
        DbApplicationRepo, DbClientRepo, DbKeyRepo, DbOAuth2AuthorizationCodeRepo,
        DbOAuth2RefreshTokenRepo, DbOAuth2UserConsentRepo, DbUserRepo,
    },
    repo::PrivateKeyKeyringRepo,
};

pub struct RouterState {
    pub api_base_uri: String,
    pub engine: db::Client,
    pub oauth2_service: Arc<
        OAuth2Service<
            DbApplicationRepo,
            DbClientRepo,
            DbOAuth2AuthorizationCodeRepo,
            DbOAuth2RefreshTokenRepo,
            DbUserRepo,
            DbOAuth2UserConsentRepo,
            DbKeyRepo,
            PrivateKeyKeyringRepo,
        >,
    >,
}
