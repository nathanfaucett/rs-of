#![allow(
    clippy::absolute_paths,
    reason = "generated paths avoid schema-name collisions"
)]
#![allow(
    clippy::doc_markdown,
    reason = "OpenAPI descriptions are preserved verbatim"
)]
#![allow(
    clippy::enum_variant_names,
    clippy::struct_field_names,
    reason = "generated public names preserve OpenAPI schema names"
)]
#![allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "generated operations preserve their OpenAPI parameter contracts"
)]

extern crate reqwest as _;
extern crate serde as _;
extern crate serde_json as _;
extern crate serde_repr as _;
extern crate url as _;

pub mod apis;
pub mod models;
