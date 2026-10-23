use std::sync::Arc;

use db::{Database, SqlTranslator};
use idp_model::{
    contract::{
        IdpSignerRecord, JwkPublic, JwkPublicParameters, JwsAlgorithm, KeyUse, ReplicaMembership,
    },
    model::Id,
};
use idp_service::replica::DbReplicaSignerRepo;

const INSTALLATION: &str = "https://installation.example";
pub(super) async fn sql(database: &Database, statement: &str) -> Vec<db::QueryResult> {
    database
        .client()
        .translate_and_execute(statement, &SqlTranslator)
        .await
        .expect("public SQL fixture")
}

pub(super) async fn enroll(idp: Arc<Database>, member: Id, endpoint: iroh::EndpointId, key: Id) {
    DbReplicaSignerRepo::new(idp.client())
        .enroll(
            &ReplicaMembership {
                installation_id: INSTALLATION.into(),
                member_id: member,
                endpoint_id: endpoint.to_string(),
                issuer: INSTALLATION.into(),
                approved_at: 1,
                revoked_at: None,
            },
            &IdpSignerRecord {
                member_id: member,
                key_id: key,
                issuer: INSTALLATION.into(),
                public_jwk: JwkPublic {
                    kid: key.to_string(),
                    r#use: KeyUse::Signature,
                    alg: JwsAlgorithm::ES256K,
                    params: JwkPublicParameters::Ec {
                        crv: "secp256k1".into(),
                        x: "eb5mfvncu6xVoGKVzocLBwKb/NstzijZWfKBWxb4F5g".into(),
                        y: "SDradyajxGVdpPv8DhEIqP0XtEimhVQZnEfQj/sQ1Lg".into(),
                    },
                },
                approved_at: Some(1),
                revoked_at: None,
                expires_at: None,
            },
        )
        .await
        .expect("persist public peer approval");
}
