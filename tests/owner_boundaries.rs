#![cfg(unix)]

#[path = "owner_boundaries/device_authority_tests.rs"]
mod device_authority_tests;
mod owner_composition_tests;
#[path = "owner_boundaries/permission_http_tests.rs"]
mod permission_http_tests;
#[path = "owner_boundaries/policy_cleanup_http_tests.rs"]
mod policy_cleanup_http_tests;
#[path = "owner_boundaries/residency_http_tests.rs"]
mod residency_http_tests;
mod support;
