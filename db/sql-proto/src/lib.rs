#![forbid(unsafe_code)]
#![allow(clippy::double_must_use, reason = "generated protobuf code")]
#![allow(
    clippy::allow_attributes_without_reason,
    reason = "generated protobuf code includes allow(clippy::all) without reason"
)]
#![allow(
    clippy::absolute_paths,
    reason = "generated protobuf code uses absolute paths"
)]

#[cfg(feature = "file-descriptor-set")]
pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("db");

tonic::include_proto!("db");
