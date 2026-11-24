#[cfg(not(feature = "std"))]
use alloc::string::String;

use serde::{Deserialize, Serialize};

use crate::idp::contract::jwk_public_parameters::JwkPublicParameters;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(tag = "kty")]
pub enum JwkPrivateParameters {
    Rsa {
        n: String,
        e: String,
        d: String,
        p: String,
        q: String,
    },
    Ec {
        crv: String,
        x: String,
        y: String,
        d: String,
    },
    Opk {
        crv: String,
        x: String,
        d: String,
    },
    Oct {
        k: String,
    },
}

impl From<JwkPrivateParameters> for JwkPublicParameters {
    fn from(val: JwkPrivateParameters) -> Self {
        match val {
            JwkPrivateParameters::Rsa { n, e, .. } => Self::Rsa { n, e },
            JwkPrivateParameters::Ec { crv, x, y, .. } => Self::Ec { crv, x, y },
            JwkPrivateParameters::Opk { crv, x, .. } => Self::Opk { crv, x },
            JwkPrivateParameters::Oct { .. } => {
                unreachable!("Octet keys are not supported in Jwk Public Parameters")
            }
        }
    }
}
