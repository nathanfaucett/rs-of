#![allow(
    clippy::absolute_paths,
    clippy::allow_attributes_without_reason,
    clippy::clone_on_ref_ptr,
    clippy::default_trait_access,
    clippy::derive_partial_eq_without_eq,
    clippy::doc_markdown,
    clippy::double_must_use,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::too_many_lines,
    clippy::trivially_copy_pass_by_ref,
    reason = "generated protobuf code"
)]

tonic::include_proto!("kvdb.v2");

#[cfg(feature = "file-descriptor-set")]
pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("kv");
