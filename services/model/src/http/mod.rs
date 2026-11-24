#![allow(
    clippy::doc_link_with_quotes,
    clippy::doc_markdown,
    clippy::too_long_first_doc_paragraph,
    reason = "OpenAPI descriptions are preserved verbatim"
)]
#![allow(
    clippy::enum_variant_names,
    clippy::module_inception,
    clippy::similar_names,
    clippy::struct_field_names,
    reason = "Generated public names preserve OpenAPI schema names"
)]
#![allow(
    clippy::option_option,
    reason = "Nested options distinguish absent and null OpenAPI fields"
)]
#![allow(
    clippy::too_many_arguments,
    reason = "Generated constructors retain every required OpenAPI field"
)]

pub mod device;
pub mod idp;
pub mod management;
pub mod storage;
pub mod storage_api;
