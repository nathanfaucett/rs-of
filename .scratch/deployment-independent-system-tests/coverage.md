# Unified assertion coverage inventory (ticket 01)

Baseline: `191962716` (integration tip at claim). Categories describe assertions,
not filenames. No existing scenario is deleted as redundant by this slice.
Names below are the baseline test identifiers, including ignored subprocess entries.
They are a search index for each assertion body, not a claim that every test was run.

## Migrated coverage

| Baseline assertion body | External consumer | Preserved observations |
| --- | --- | --- |
| `http_server_tests::fresh_host_publishes_complete_openapi_without_provisioning` | `crates/unified-server/tests/http_host.rs` | Exact exported schemas and public bases; stable unavailable/unknown/method routing; setup-required errors; CORS DELETE; no owner, endpoint, credential, provisioning, or TLS material created |
| `http_server_tests::{http_setup_create_and_reopen_without_certificates,http_setup_child}` | `tests/http_host.rs` | Isolated provider child with deadline; absent/wrong admission, hostile Origin, accepted Create, completed exact retry, wrong-password denial; retained HTTP reopen and health; CLI create/verify/serve without TLS |
| `setup_runtime_tests::owner_routes_stay_mounted_when_runtime_becomes_available_or_unavailable` | `tests/owner_routes.rs` | Public worker publish/withdraw cycle; owner statuses, stable metadata and unknown-route behavior |
| `setup_host_tests::fresh_host_awaits_choice_without_installation_or_credentials_and_reopens_exactly` | `tests/setup_host.rs` | Neutral identity, exclusive host guard, exact retained admission/CA, missing-admission fail-closed without regeneration |
| `setup_host_tests::interrupted_host_settings_are_not_fresh_generation_permission` | `tests/setup_host.rs` | Reopened metadata cannot authorize fresh TLS/admission generation |
| `bootstrap_config_tests::ordinary_create_derives_and_retains_all_public_host_settings` | `tests/bootstrap_config.rs` | Independent public address/issuer, distinct owner directories, stable reopen, duplicate Create refusal |
| `bootstrap_config_tests::join_settings_require_peer_evidence_not_an_invented_installation` | `tests/bootstrap_config.rs` | Join requires peer evidence, retains issuer/peer, missing host identity cannot be recreated |
| `generated_transport_tests` (four tests) | `crates/idp-client/tests/transport.rs` | Streaming overflow before EOF, bounded errors/requests/headers/time, malformed JSON, redirects not followed, HTTPS-only defaults, untrusted TLS rejection |
| Existing `tests/composition.rs` (unchanged) | Same external consumer | Four supplied owner routers only, no invented/doubled routes; real owner builders mount hosted features; unauthenticated Device Join denied |

Raw requests in migrated tests are intentional exact routing/status/header/schema
assertions (including 202 acceptance and denial statuses). Ordinary readiness,
completed Create retry and health use generated IdP models/operations with the
actual owner base and a bounded transport. Public filesystem paths are observed for
host lifecycle/non-generation assertions; no database is inspected or ready state
injected into a full-Node scenario. The router publication test is explicitly a
Unified composition test, not Node readiness qualification.

The HTTP fixture now installs the public mock credential store directly: it no
longer imports the private fault-injection `SecretStore` or calls private endpoint
lookup configuration. The fault-injection store remains for tests that require it.
Removed the old generated-transport module declaration and relocated its entire
fixture to its owner. No public production API, visibility, or runtime state added.

## Retained/deferred coverage and replacement boundaries

- **Private unit**: configuration parsing, metadata/journal damage, permissions,
  symlink/FIFO/lock checks, publication checkpoints, interrupted setup, candidate
  histories, private verifier/signing-material exclusion, and response-time fault
  injection remain beside implementation. These depend on deliberate corruption,
  journal formats, mock failures, or paused execution; black-box equivalents must
  not replace those assertions.
- **Mixed public/private**: `local_provisioning_tests`, `hosted_setup_integration_tests`,
  `https_bootstrap_tests`, and `runtime.rs` still contain broader scenarios. Their
  public Create/Join/login/offline/revocation assertions are candidates for ticket
  02+ shared scenarios; their corruption/recovery/publication assertions remain
  private. Existing tests remain enabled until assertion-equivalent replacements
  identify exact test bodies. In particular `runtime::builds_three_prefixed_services_with_one_endpoint`
  seeds identity/Device SQL and checks internal owner wiring: external routing is
  now covered by composition/owner_routes, but its native protocol and authorization
  assertions cannot be deleted in favor of those smaller tests.
- **Owner-only**: generated transport is relocated now. Permission, Device authority,
  residency and copy-retirement tests still combine owner repositories, shared
  fixtures and/or runtime authorizers. Move those into their owning service/server
  after extracting fixture dependencies, not by adding a server-to-server dependency
  or exposing `runtime::owner_replication_authorizer`. They are retained unchanged.
- **Shared Node scenarios**: full-node, native API, API Join and bootstrap scenarios
  exercise Create/Join, source-offline reopen, ordinary client authorization,
  consumption/restriction/revocation and sync. Ticket 02+ owns new concrete launch
  fixtures and shared assertion bodies outside Unified. Do not delete the old
  protocol/security assertions merely because fresh Create passes in two layouts.
- **Deployment-specific**: remote identity/Device SQL consumers and independent
  owner composition tests verify shared remote owner persistence, reset boundaries,
  owner-scoped admission and HTTPS bindings. They are not independent persisted-Node
  Join qualification and are retained separately.
- **Hooks**: `device_engine`/`full_node_test_devices`, `full_node_test_identity`,
  `full_node_test_server`, `node_join_source_for_test`, and
  `trace_bootstrap_http_stages` still have private recovery/security/bootstrap users.
  They are not obsolete in this slice. Delete each when its final fault-injection
  or white-box caller is replaced; do not export it to integration consumers.
- **Support files**: `full_node_runtime_fixture` serves the private runtime tests;
  `full_node_probe` is a targeted diagnostic, not replacement qualification;
  `owner_composition_tests/reset` is fixture/recovery support. Preserve native
  WebSocket/Iroh clients and existing Rust/JavaScript generated-client qualification.

## Baseline assertion index

Category labels below give the primary assertion seam. Mixed modules use the
per-assertion exceptions above; this index includes nested/path-mounted modules.

### `crates/unified-server/src/bootstrap_config_tests.rs` — private unit / mixed (see exceptions)

- `ordinary_create_derives_and_retains_all_public_host_settings`
- `independent_installations_on_the_same_local_port_have_distinct_issuers`
- `join_settings_require_peer_evidence_not_an_invented_installation`

### `crates/unified-server/src/cli.rs` — private unit / mixed (see exceptions)

- `default_logger_filter_quiets_automerge_and_preserves_configured_levels`
- `retained_host_accepts_only_configured_ui_origin_without_rewriting_identity`
- `port_environment_requires_a_nonzero_tcp_port`
- `automatic_startup_requires_positively_absent_roots`
- `ordinary_bootstrap_has_no_required_config_names_or_password_file`

### `crates/unified-server/src/config.rs` — private unit / mixed (see exceptions)

- `explicit_bootstrap_retains_concrete_server_options_and_public_references`
- `default_unified_config_loads_without_relationship_secrets`
- `default_standalone_configs_load_without_relationship_secrets`
- `default_unified_config_supports_fresh_explicit_bootstrap_and_retained_issuer`
- `bundled_config_needs_no_relationship_secrets_and_refuses_removed_configuration`

### `crates/unified-server/src/device_authority_tests.rs` — owner-only

- `device_http_and_storage_share_isolated_authority_and_recheck_revocation`

### `crates/unified-server/src/endpoint.rs` — private unit / mixed (see exceptions)

- `existing_open_refuses_missing_identity_without_creating_state`
- `rejects_insecure_existing_key_permissions`
- `rejects_symlink_directory_ancestry_before_read_or_creation`
- `rejects_existing_symlink_key_without_changing_its_target`
- `fresh_creation_publishes_identity_and_retry_is_exact`
- `endpoint_publication_child`
- `interrupted_publication_reopens_exactly_or_refuses_missing_canonical_identity`
- `owns_one_persisted_endpoint_and_matches_idp_identity`
- `iroh_rejects_connections_after_peer_revocation`
- `rejects_invalid_existing_key_file`

### `crates/unified-server/src/full_node_probe.rs` — shared Node system / mixed private fixture

- `single_full_host_refresh_replay_and_restriction_put`

### `crates/unified-server/src/full_node_tests.rs` — shared Node system / mixed private fixture

- `two_completed_full_hosts_join_partition_reopen_and_learn_denial`
- `two_completed_full_hosts_reconcile_consumption_and_revocation`
- `two_completed_full_hosts_learn_restrictions_on_retained_sockets`

### `crates/unified-server/src/generated_transport_tests.rs` — owner-only

- `generated_transport_rejects_streaming_overflow_before_end_of_body`
- `generated_transport_bounds_errors_and_rejects_malformed_json_and_redirects`
- `generated_transport_honors_request_and_time_bounds_and_https_only_default`
- `generated_transport_verifies_tls_instead_of_accepting_unknown_certificates`

### `crates/unified-server/src/hosted_setup_integration_tests.rs` — shared Node system / mixed private fixture

- `retired_join_reopen_refuses_mismatched_or_missing_intent_without_owner_mutation`
- `partial_credentials_reopen_preserves_recovery_for_create_and_join`
- `production_api_create_authentication_and_offline_reopen`
- `api_create_child`

### `crates/unified-server/src/http_server_tests.rs` — Unified public integration

- `fresh_host_publishes_complete_openapi_without_provisioning`
- `http_setup_create_and_reopen_without_certificates`
- `http_setup_child`
- `http_setup_origin_is_exact_and_loopback_only`

### `crates/unified-server/src/https_bootstrap_tests.rs` — shared Node system / mixed private fixture

- `production_https_create_join_and_offline_reopen_with_isolated_native_roots`
- `production_https_local_create_and_completed_verification_with_isolated_native_roots`
- `hosted_join_cancellation_exact_retry_and_offline_reopen`
- `production_api_join_interrupted_retry_and_rejection`
- `production_api_join_completion_and_offline_reopen`
- `production_setup_status_tracks_current_local_revocation`
- `production_api_join_complete_unretired_finalization`
- `production_https_bootstrap_child`
- `unsupported_tls_address_refuses_before_any_retained_material`

### `crates/unified-server/src/local_provisioning_tests.rs` — private unit / mixed (see exceptions)

- `identity_settings_errors_preserve_host_validation_order_without_writing_intent`
- `in_memory_password_create_retains_only_its_verifier`
- `live_credential_guard_refuses_missing_or_changed_intent_before_provider_work`
- `old_one_two_and_four_relationship_references_deny_without_rewriting_or_provider_work`
- `old_one_two_and_four_relationship_create_and_join_intents_deny_unchanged`
- `host_bootstrap_reopens_without_relationship_secrets_or_provider_work`
- `bundled_management_direct_idp_rechecks_registration_and_device_owner_without_http`
- `byte_mismatch_diagnostics_do_not_dump_contents`
- `local_provisioning_restores_stopped_same_node_filesystem_with_retained_provider`
- `host_references_drive_complete_owner_create_and_offline_reopen`
- `join_intent_is_public_exact_and_never_startup_readiness`
- `local_provisioning_creates_exact_host_and_reopens_offline`
- `local_provisioning_continues_retained_attempt_and_refuses_changed_identity`
- `local_provisioning_existing_cli_host_without_journal_starts_and_reopens`
- `local_provisioning_lost_management_after_late_failure_denies_without_recreation`
- `local_provisioning_lost_storage_after_runtime_initialization_denies_without_recreation`
- `local_provisioning_missing_attempted_signer_denies_without_replacement`
- `local_provisioning_reuses_pre_enrollment_signer_after_readback_interruption`
- `local_provisioning_rejects_fifo_password_before_opening_it`
- `local_provisioning_rejects_unprotected_input_and_missing_or_conflicting_intent`
- `concrete_host_settings_preserve_the_durable_intent_when_normalized`
- `retained_create_journal_keeps_its_flat_schema_and_exact_retry_checks`

### `crates/unified-server/src/native_api_qualification_tests.rs` — shared Node system / mixed private fixture

- `bundled_storage_direct_idp_checks_current_bearer_registration_and_signer_offline`
- `bundled_storage_direct_idp_preserves_real_local_owner_gate`
- `native_catalog_response_denies_learned_application_reassignment_without_rollback`
- `native_catalog_paused_local_owner_rechecks_before_response_without_rollback`
- `independent_catalog_paused_introspection_timeout_returns_503_without_rollback`
- `native_catalog_device_restrictions_do_not_revoke_bearer_results`
- `native_socket_device_restriction_preserves_bearer_response_and_next_operation`
- `native_socket_response_binds_namespace_even_when_resource_id_exists_in_new_application`
- `native_socket_response_denies_application_reassignment_and_retains_completed_write`

### `crates/unified-server/src/native_api_tests.rs` — shared Node system / mixed private fixture

- `native_owner_cutover_captures_initial_secrets_and_reuses_reopened_owner`
- `native_local_token_read_response_denies_learned_revocation_before_transmission`
- `native_local_tokens_bound_api_actions_resource_kind_and_existing_socket_operations`
- `native_local_token_authorized_write_survives_revoked_response_and_next_operation_denial`
- `independent_idp_service_loss_denies_response_and_api_without_cached_fallback`
- `native_catalog_create_response_denies_learned_user_deletion_without_rollback`
- `native_catalog_all_result_kinds_recheck_after_execution_and_retain_mutations`
- `generated_rust_and_built_javascript_use_actual_owner_routes`

### `crates/unified-server/src/node_bootstrap_interruption_tests.rs` — private unit / mixed (see exceptions)

- `bootstrap_current_boundaries_preserve_admitted_import_and_native_retry`
- `desktop_bootstrap_orchestration_completes_both_public_contexts_and_retries_offline`
- `production_durable_transitions_preserve_imports_and_verified_readiness_on_reopen`

### `crates/unified-server/src/node_join_bootstrap_tests.rs` — shared Node system / mixed private fixture

- `authorized_node_bootstrap_imports_both_contexts_and_reopens_offline`
- `owner_cutover_node_bootstrap_reopens_offline`

### `crates/unified-server/src/owner_composition_tests/create.rs` — deployment-specific system

- `production_independent_create_and_offline_reopen`
- `production_independent_create_legacy_record_reset`
- `independent_create_child`

### `crates/unified-server/src/owner_composition_tests/join.rs` — deployment-specific system

- `production_independent_join_and_source_offline_reopen`
- `production_independent_join_legacy_record_reset`
- `independent_join_child`

### `crates/unified-server/src/owner_composition_tests/operation.rs` — deployment-specific system

- `production_independent_signer_free_operation_and_delayed_authority_reopen`
- `operation_child`

### `crates/unified-server/src/owner_composition_tests/permissions.rs` — deployment-specific system

- `independent_permission_production_management_https`
- `independent_permission_management_https_child`

### `crates/unified-server/src/owner_composition_tests/remote_setup.rs` — deployment-specific system

- `remote_create_join_reopen_and_device_reset_preserve_shared_installation`
- `remote_setup_child`

### `crates/unified-server/src/owner_composition_tests/transfer.rs` — deployment-specific system

- `stale_cleanup_cannot_clear_current_binding_and_exact_retry_stays_restricted`
- `original_owner_return_keeps_retirement_pending_after_stopped_reopen`
- `independent_transfer_production_https`
- `independent_transfer_https_child`

### `crates/unified-server/src/permission_http_tests.rs` — owner-only

- `normal_permission_http_administration`
- `normal_identity_and_infrastructure_lifecycle`
- `shared_identity_management_writes_change_idp_authorization`

### `crates/unified-server/src/policy_cleanup_http_tests.rs` — owner-only

- `real_policy_http_chain_retires_copies_reselects_generation_and_gates_transfer`

### `crates/unified-server/src/remote_device_process_tests.rs` — deployment-specific system

- `remote_device_process_admission_and_consumer_reset_preserve_shared_rows`
- `remote_device_child`

### `crates/unified-server/src/remote_identity_process_tests.rs` — deployment-specific system

- `independent_processes_share_remote_identity_and_consumer_reset_preserves_rows`
- `remote_identity_child`

### `crates/unified-server/src/residency_http_tests.rs` — owner-only

- `authorized_whole_filesystem_residency_uses_public_storage_owner`
- `public_residency_read_reports_durable_mode_after_native_reopen`
- `overlapping_residency_puts_serialize_only_the_exact_copy_and_reopen_consistently`
- `residency_denials_preserve_mode_and_cleanup_fences`
- `residency_rechecks_learned_user_denial_before_owner_mutation`
- `residency_rechecks_deleted_target_before_owner_mutation`
- `residency_owner_write_failure_preserves_prior_policy_and_content`

### `crates/unified-server/src/runtime.rs` — shared Node system / mixed private fixture

- `host_api_base_must_match_the_loopback_listener`
- `builds_three_prefixed_services_with_one_endpoint`
- `native_membership_restores_offline_and_learned_revocation_removes_admission`

### `crates/unified-server/src/runtime_peer_refresh_tests.rs` — private unit / mixed (see exceptions)

- `peer_refresh_is_idle_and_coalesces_both_sources`
- `peer_refresh_retries_failures_without_another_commit`
- `peer_refresh_tracks_real_sql_commits_and_read_commits_are_silent`

### `crates/unified-server/src/security_candidate_tests.rs` — private unit / mixed (see exceptions)

- `marker3_revalidates_semantic_recovery_and_preserves_local_contender`
- `approved_endpoints_without_idp_signers_exchange_and_device_revocation_denies`
- `approved_devices_reject_conflicted_public_device_bindings`
- `marker3_persists_pending_conflicts_and_tombstones_without_trust`
- `marker3_rejects_correlated_rebinding_of_authenticated_session_peer`
- `management_rejects_overwritten_invalid_device_history_before_export_or_apply`
- `marker3_rejects_semantic_candidates_in_both_directions_and_roles`
- `marker3_rejects_semantic_candidates_in_known_row_deltas`

### `crates/unified-server/src/security_history_tests.rs` — private unit / mixed (see exceptions)

- `private_jwk_remains_in_a_losing_conflict_behind_a_valid_public_projection`
- `marker3_rejects_private_jwk_history_before_disclosing_to_unvalidated_peer`
- `marker3_rejects_unvalidated_incoming_private_jwk_history_before_apply`
- `marker3_rejects_private_jwk_in_an_unvalidated_full_state`
- `marker3_rejects_private_jwk_schema_defaults_before_disclosure`
- `marker3_revalidates_late_private_history_in_recovery_on_both_roles`
- `marker3_rejects_overwritten_raw_grant_hashes_in_both_directions_and_roles`
- `marker3_valid_public_key_and_hash_histories_remain_syncable`

### `crates/unified-server/src/security_sync_tests.rs` — private unit / mixed (see exceptions)

- `authenticated_equal_peers_exchange_scoped_history_without_local_verifiers`

### `crates/unified-server/src/setup_host_tests.rs` — private unit / mixed (see exceptions)

- `fresh_host_awaits_choice_without_installation_or_credentials_and_reopens_exactly`
- `interrupted_host_settings_are_not_fresh_generation_permission`
- `neutral_production_history_is_required_without_changing_active_host_metadata`
- `explicit_configuration_classifies_absence_and_never_selects_an_issuer`
- `setup_https_uses_isolated_native_roots_and_never_installs_real_trust`
- `setup_https_child`

### `crates/unified-server/src/setup_runtime_tests.rs` — Unified public integration

- `owner_routes_stay_mounted_when_runtime_becomes_available_or_unavailable`

### `crates/unified-server/src/unsupported_bootstrap_tests.rs` — private unit / mixed (see exceptions)

- `unsupported_platform_rejects_all_bootstrap_constructors_before_creating_state`


## Ticket03 lifecycle coverage

`tests/system/create.rs` shares retained restart and authorized Join assertions
across actual Unified and independent CLI processes. `tests/system_create.rs`
executes both layouts plus explicit local-owner Create/restart. Retained public
readiness, endpoint identity, pre-restart resource reads, new authorized resource
operations, anonymous denials and exact Management permission decisions are tested.
Independent Join checks each host's capability and correlated owner evidence,
imports into independently owned target stores, registers target-local Management
credentials ordinarily, stops the source and reopens the target without reseeding.

The former Management local-owner connection blocker is addressed by the explicit
user-approved socket selector, not by sharing source SQL or a fixture proxy.
No legacy complete test body is wholly replaced: `api_join_tests` also qualifies
interrupted/rejected/exact-retry/finalization/publication cases; `https_bootstrap_tests`
and `hosted_setup_integration_tests` contain retained damage/intent/TLS/host cases;
`owner_composition_tests::join` includes interruption, reset and recovery; full-node
native tests include synchronization, revocation and private credential assertions.
These remain, with their existing private helpers. No newly obsolete hook was found.
See `ticket03-report.md` for actual commands, production N0 networking dependence,
prior failures and the supported qualification boundary.
