#[cfg(all(not(feature = "std"), feature = "wasm"))]
use alloc::{boxed::Box, format};
#[cfg(not(feature = "std"))]
use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    vec::Vec,
};
#[cfg(feature = "std")]
use std::collections::BTreeMap;

use thiserror::Error;

use value::Value;

use crate::Statement;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub enum QueryParams {
    Positional(Vec<Value>),
    Named(BTreeMap<String, Value>),
}

#[derive(Error, Debug)]
pub enum TranslateError {
    #[error("Missing named parameter: {0}")]
    MissingNamedParameter(String),

    #[error("cannot mix named and positional/indexed placeholders in one query")]
    MixedPlaceholderStyles,

    #[error("Error: {0}")]
    Custom(String),
}

impl TranslateError {
    #[allow(
        clippy::needless_pass_by_value,
        reason = "accepts owned errors directly and through Result::map_err"
    )]
    pub fn custom<T>(error: T) -> Self
    where
        T: ToString,
    {
        Self::Custom(error.to_string())
    }
}

pub type TranslateResult<T> = Result<T, TranslateError>;

pub trait Translator {
    fn translate_with_params(
        &self,
        query: &str,
        params: Option<&QueryParams>,
    ) -> impl Future<Output = Result<Vec<Statement>, TranslateError>> + platform::MaybeSend;

    fn translate(
        &self,
        query: &str,
    ) -> impl Future<Output = Result<Vec<Statement>, TranslateError>> + platform::MaybeSend {
        self.translate_with_params(query, None)
    }
}
