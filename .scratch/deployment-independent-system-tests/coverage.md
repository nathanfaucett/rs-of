# Unified assertion coverage inventory (final ticket05 reconciliation)

Baseline: `191962716` (ticket01 claim); final audit against integration `120f7953a`.
Categories describe assertions, not filenames. Ticket01 and ticket03 sections below
are historical slice reports. Final dispositions and ticket05 audit supersede their
prospective migration language. Ticket04 removed only its explicitly replaced
35-line public rejection/retry block; no whole mixed scenario was deleted.
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

The HTTP fixture now uses a test-local modifier-stripping adapter over the public mock credential store: it no
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

- `ordinary_create_derives_and_retains_all_public_host_settings` — **moved external**: same assertion body now in `crates/unified-server/tests/bootstrap_config.rs`; old unit body/declaration removed.
- `independent_installations_on_the_same_local_port_have_distinct_issuers` — **retained private**: same-port distinct logical issuer/retained host metadata invariant.
- `join_settings_require_peer_evidence_not_an_invented_installation` — **moved external**: same assertion body now in `crates/unified-server/tests/bootstrap_config.rs`; old unit body/declaration removed.

### `crates/unified-server/src/cli.rs` — private unit / mixed (see exceptions)

- `default_logger_filter_quiets_automerge_and_preserves_configured_levels` — **retained private CLI**: CLI argument/environment/origin/startup classification; not a common owner HTTP contract.
- `retained_host_accepts_only_configured_ui_origin_without_rewriting_identity` — **retained private CLI**: CLI argument/environment/origin/startup classification; not a common owner HTTP contract.
- `port_environment_requires_a_nonzero_tcp_port` — **retained private CLI**: CLI argument/environment/origin/startup classification; not a common owner HTTP contract.
- `automatic_startup_requires_positively_absent_roots` — **retained private CLI**: CLI argument/environment/origin/startup classification; not a common owner HTTP contract.
- `ordinary_bootstrap_has_no_required_config_names_or_password_file` — **retained private CLI**: CLI argument/environment/origin/startup classification; not a common owner HTTP contract.

### `crates/unified-server/src/config.rs` — private unit / mixed (see exceptions)

- `explicit_bootstrap_retains_concrete_server_options_and_public_references` — **retained private**: configuration parsing/defaults/removal validation and validation ordering.
- `default_unified_config_loads_without_relationship_secrets` — **retained private**: configuration parsing/defaults/removal validation and validation ordering.
- `default_standalone_configs_load_without_relationship_secrets` — **retained private**: configuration parsing/defaults/removal validation and validation ordering.
- `default_unified_config_supports_fresh_explicit_bootstrap_and_retained_issuer` — **retained private**: configuration parsing/defaults/removal validation and validation ordering.
- `bundled_config_needs_no_relationship_secrets_and_refuses_removed_configuration` — **retained private**: configuration parsing/defaults/removal validation and validation ordering.

### `crates/unified-server/src/device_authority_tests.rs` — owner-only

- `device_http_and_storage_share_isolated_authority_and_recheck_revocation` — **retained mixed owner wiring**: isolated Device authority and live Storage rechecks require injected owner repository; public User deletion is not replacement.

### `crates/unified-server/src/endpoint.rs` — private unit / mixed (see exceptions)

- `existing_open_refuses_missing_identity_without_creating_state` — **retained private/native**: persisted endpoint identity, permissions/ancestry or publication/revocation boundary, not merely Node readiness.
- `rejects_insecure_existing_key_permissions` — **retained private/native**: persisted endpoint identity, permissions/ancestry or publication/revocation boundary, not merely Node readiness.
- `rejects_symlink_directory_ancestry_before_read_or_creation` — **retained private/native**: persisted endpoint identity, permissions/ancestry or publication/revocation boundary, not merely Node readiness.
- `rejects_existing_symlink_key_without_changing_its_target` — **retained private/native**: persisted endpoint identity, permissions/ancestry or publication/revocation boundary, not merely Node readiness.
- `fresh_creation_publishes_identity_and_retry_is_exact` — **retained private/native**: persisted endpoint identity, permissions/ancestry or publication/revocation boundary, not merely Node readiness.
- `endpoint_publication_child` — **retained private/native**: persisted endpoint identity, permissions/ancestry or publication/revocation boundary, not merely Node readiness.
- `interrupted_publication_reopens_exactly_or_refuses_missing_canonical_identity` — **retained private/native**: persisted endpoint identity, permissions/ancestry or publication/revocation boundary, not merely Node readiness.
- `owns_one_persisted_endpoint_and_matches_idp_identity` — **retained private/native**: persisted endpoint identity, permissions/ancestry or publication/revocation boundary, not merely Node readiness.
- `iroh_rejects_connections_after_peer_revocation` — **retained private/native**: persisted endpoint identity, permissions/ancestry or publication/revocation boundary, not merely Node readiness.
- `rejects_invalid_existing_key_file` — **retained private/native**: persisted endpoint identity, permissions/ancestry or publication/revocation boundary, not merely Node readiness.

### `crates/unified-server/src/full_node_probe.rs` — shared Node system / mixed private fixture

- `single_full_host_refresh_replay_and_restriction_put` — **retained private diagnostic**: private reserved-Installation restriction grants plus refresh replay; shared process denial cannot seed these grants.

### `crates/unified-server/src/full_node_tests.rs` — shared Node system / mixed private fixture

- `two_completed_full_hosts_join_partition_reopen_and_learn_denial` — **retained mixed live protocol**: private signer/provider inventory, partition/reconcile and per-host grant race; shared Join/import alone is not replacement.
- `two_completed_full_hosts_reconcile_consumption_and_revocation` — **partly shared / live reconciliation retained**: shared Join learns initial consumed/revoked history with positive control/offline replay denial; original mutates grants AFTER Join then reconnects IdP SecurityProtocol and checks remote denial/local races; no public process sync trigger.
- `two_completed_full_hosts_learn_restrictions_on_retained_sockets` — **retained live Device reconciliation**: privately grant reserved Installation restrictions, mutate opposite Devices during partition, reconnect Device SecurityProtocol, deny retained sockets, restore and verify unchanged bytes; public User deletion is NOT this behavior.

### `crates/unified-server/src/generated_transport_tests.rs` — owner-only

- `generated_transport_rejects_streaming_overflow_before_end_of_body` — **moved external**: same assertion body now in `crates/idp-client/tests/transport.rs`; old unit body/declaration removed.
- `generated_transport_bounds_errors_and_rejects_malformed_json_and_redirects` — **moved external**: same assertion body now in `crates/idp-client/tests/transport.rs`; old unit body/declaration removed.
- `generated_transport_honors_request_and_time_bounds_and_https_only_default` — **moved external**: same assertion body now in `crates/idp-client/tests/transport.rs`; old unit body/declaration removed.
- `generated_transport_verifies_tls_instead_of_accepting_unknown_certificates` — **moved external**: same assertion body now in `crates/idp-client/tests/transport.rs`; old unit body/declaration removed.

### `crates/unified-server/src/hosted_setup_integration_tests.rs` — shared Node system / mixed private fixture

- `retired_join_reopen_refuses_mismatched_or_missing_intent_without_owner_mutation` — **retained private recovery**: retained intent/credential damage and exact recovery observations remain beyond public Create/restart.
- `partial_credentials_reopen_preserves_recovery_for_create_and_join` — **retained private recovery**: retained intent/credential damage and exact recovery observations remain beyond public Create/restart.
- `production_api_create_authentication_and_offline_reopen` — **partly shared / mixed remainder retained**: common Create/login/offline readiness now shared; exact setup wire, owner state/private retained recovery and isolated child remain.
- `api_create_child` — **retained isolated child**: parent production_api_create_authentication_and_offline_reopen explicitly runs provider-isolated private/wire checks.

### `crates/unified-server/src/http_server_tests.rs` — Unified public integration

- `fresh_host_publishes_complete_openapi_without_provisioning` — **moved external**: same assertion body now in `crates/unified-server/tests/http_host.rs`; old unit body/declaration removed.
- `http_setup_create_and_reopen_without_certificates` — **moved external**: same assertion body now in `crates/unified-server/tests/http_host.rs`; old unit body/declaration removed.
- `http_setup_child` — **moved external**: same assertion body now in `crates/unified-server/tests/http_host.rs`; old unit body/declaration removed.
- `http_setup_origin_is_exact_and_loopback_only` — **retained private**: direct private loopback/origin validator; external exact hostile-Origin wire denial also retained.

### `crates/unified-server/src/https_bootstrap_tests.rs` — shared Node system / mixed private fixture

- `production_https_create_join_and_offline_reopen_with_isolated_native_roots` — **retained mixed Unified TLS/recovery**: native-root/provider isolation, retained private admission/TLS/host lifecycle or publication fences not wholly replaced by shared process Join.
- `production_https_local_create_and_completed_verification_with_isolated_native_roots` — **retained mixed Unified TLS/recovery**: native-root/provider isolation, retained private admission/TLS/host lifecycle or publication fences not wholly replaced by shared process Join.
- `hosted_join_cancellation_exact_retry_and_offline_reopen` — **retained mixed Unified TLS/recovery**: native-root/provider isolation, retained private admission/TLS/host lifecycle or publication fences not wholly replaced by shared process Join.
- `production_api_join_interrupted_retry_and_rejection` — **partly replaced / private remainder renamed**: shared recovery.rs replaces the 35-line public rejection/exact-request/retry block in both layouts; current production_api_join_interruption_preserves_private_host_intent retains host purpose, issuer/peer, TLS/admission, exclusive guard and offer release.
- `production_api_join_completion_and_offline_reopen` — **retained mixed Unified TLS/recovery**: native-root/provider isolation, retained private admission/TLS/host lifecycle or publication fences not wholly replaced by shared process Join.
- `production_setup_status_tracks_current_local_revocation` — **retained private publication/readiness**: remove reserved context grant through private owner and prove current readiness withdrawal/restoration without blanket route gate.
- `production_api_join_complete_unretired_finalization` — **retained private crash recovery**: restore pre-retirement admission bytes to emulate completed-but-unretired crash; public interruption cannot replace that exact fence.
- `production_https_bootstrap_child` — **retained mixed Unified TLS/recovery**: native-root/provider isolation, retained private admission/TLS/host lifecycle or publication fences not wholly replaced by shared process Join.
- `unsupported_tls_address_refuses_before_any_retained_material` — **retained mixed Unified TLS/recovery**: native-root/provider isolation, retained private admission/TLS/host lifecycle or publication fences not wholly replaced by shared process Join.

### `crates/unified-server/src/local_provisioning_tests.rs` — private unit / mixed (see exceptions)

- `identity_settings_errors_preserve_host_validation_order_without_writing_intent` — **retained private provisioning**: retained intent/journal/provider/owner lifecycle or precise failure boundary, not equivalent to public readiness.
- `in_memory_password_create_retains_only_its_verifier` — **retained private provisioning**: retained intent/journal/provider/owner lifecycle or precise failure boundary, not equivalent to public readiness.
- `live_credential_guard_refuses_missing_or_changed_intent_before_provider_work` — **retained private provisioning**: retained intent/journal/provider/owner lifecycle or precise failure boundary, not equivalent to public readiness.
- `old_one_two_and_four_relationship_references_deny_without_rewriting_or_provider_work` — **retained private provisioning**: retained intent/journal/provider/owner lifecycle or precise failure boundary, not equivalent to public readiness.
- `old_one_two_and_four_relationship_create_and_join_intents_deny_unchanged` — **retained private provisioning**: retained intent/journal/provider/owner lifecycle or precise failure boundary, not equivalent to public readiness.
- `host_bootstrap_reopens_without_relationship_secrets_or_provider_work` — **retained private provisioning**: retained intent/journal/provider/owner lifecycle or precise failure boundary, not equivalent to public readiness.
- `bundled_management_direct_idp_rechecks_registration_and_device_owner_without_http` — **retained private provisioning**: retained intent/journal/provider/owner lifecycle or precise failure boundary, not equivalent to public readiness.
- `byte_mismatch_diagnostics_do_not_dump_contents` — **retained private provisioning**: retained intent/journal/provider/owner lifecycle or precise failure boundary, not equivalent to public readiness.
- `local_provisioning_restores_stopped_same_node_filesystem_with_retained_provider` — **retained private provisioning**: retained intent/journal/provider/owner lifecycle or precise failure boundary, not equivalent to public readiness.
- `host_references_drive_complete_owner_create_and_offline_reopen` — **retained private provisioning**: retained intent/journal/provider/owner lifecycle or precise failure boundary, not equivalent to public readiness.
- `join_intent_is_public_exact_and_never_startup_readiness` — **retained private provisioning**: retained intent/journal/provider/owner lifecycle or precise failure boundary, not equivalent to public readiness.
- `local_provisioning_creates_exact_host_and_reopens_offline` — **retained private provisioning**: retained intent/journal/provider/owner lifecycle or precise failure boundary, not equivalent to public readiness.
- `local_provisioning_continues_retained_attempt_and_refuses_changed_identity` — **retained private provisioning**: retained intent/journal/provider/owner lifecycle or precise failure boundary, not equivalent to public readiness.
- `local_provisioning_existing_cli_host_without_journal_starts_and_reopens` — **retained private provisioning**: retained intent/journal/provider/owner lifecycle or precise failure boundary, not equivalent to public readiness.
- `local_provisioning_lost_management_after_late_failure_denies_without_recreation` — **retained private provisioning**: retained intent/journal/provider/owner lifecycle or precise failure boundary, not equivalent to public readiness.
- `local_provisioning_lost_storage_after_runtime_initialization_denies_without_recreation` — **retained private provisioning**: retained intent/journal/provider/owner lifecycle or precise failure boundary, not equivalent to public readiness.
- `local_provisioning_missing_attempted_signer_denies_without_replacement` — **retained private provisioning**: retained intent/journal/provider/owner lifecycle or precise failure boundary, not equivalent to public readiness.
- `local_provisioning_reuses_pre_enrollment_signer_after_readback_interruption` — **retained private provisioning**: retained intent/journal/provider/owner lifecycle or precise failure boundary, not equivalent to public readiness.
- `local_provisioning_rejects_fifo_password_before_opening_it` — **retained private provisioning**: retained intent/journal/provider/owner lifecycle or precise failure boundary, not equivalent to public readiness.
- `local_provisioning_rejects_unprotected_input_and_missing_or_conflicting_intent` — **retained private provisioning**: retained intent/journal/provider/owner lifecycle or precise failure boundary, not equivalent to public readiness.
- `concrete_host_settings_preserve_the_durable_intent_when_normalized` — **retained private provisioning**: retained intent/journal/provider/owner lifecycle or precise failure boundary, not equivalent to public readiness.
- `retained_create_journal_keeps_its_flat_schema_and_exact_retry_checks` — **retained private provisioning**: retained intent/journal/provider/owner lifecycle or precise failure boundary, not equivalent to public readiness.

### `crates/unified-server/src/native_api_qualification_tests.rs` — shared Node system / mixed private fixture

- `bundled_storage_direct_idp_checks_current_bearer_registration_and_signer_offline` — **retained owner/fault qualification**: private owner/session/catalog scheduling and namespace authority; common process denial does not reproduce response-time fences.
- `bundled_storage_direct_idp_preserves_real_local_owner_gate` — **retained owner/fault qualification**: private owner/session/catalog scheduling and namespace authority; common process denial does not reproduce response-time fences.
- `native_catalog_response_denies_learned_application_reassignment_without_rollback` — **retained owner/fault qualification**: private owner/session/catalog scheduling and namespace authority; common process denial does not reproduce response-time fences.
- `native_catalog_paused_local_owner_rechecks_before_response_without_rollback` — **retained owner/fault qualification**: private owner/session/catalog scheduling and namespace authority; common process denial does not reproduce response-time fences.
- `independent_catalog_paused_introspection_timeout_returns_503_without_rollback` — **retained owner/fault qualification**: private owner/session/catalog scheduling and namespace authority; common process denial does not reproduce response-time fences.
- `native_catalog_device_restrictions_do_not_revoke_bearer_results` — **retained owner/fault qualification**: private owner/session/catalog scheduling and namespace authority; common process denial does not reproduce response-time fences.
- `native_socket_device_restriction_preserves_bearer_response_and_next_operation` — **retained owner/fault qualification**: private owner/session/catalog scheduling and namespace authority; common process denial does not reproduce response-time fences.
- `native_socket_response_binds_namespace_even_when_resource_id_exists_in_new_application` — **retained owner/fault qualification**: private owner/session/catalog scheduling and namespace authority; common process denial does not reproduce response-time fences.
- `native_socket_response_denies_application_reassignment_and_retains_completed_write` — **retained owner/fault qualification**: private owner/session/catalog scheduling and namespace authority; common process denial does not reproduce response-time fences.

### `crates/unified-server/src/native_api_tests.rs` — shared Node system / mixed private fixture

- `native_owner_cutover_captures_initial_secrets_and_reuses_reopened_owner` — **retained mixed owner/protocol**: private authority setup, catalog/session gates and exact namespace/result matrix; shared filesystem denial is only partial replacement.
- `native_local_token_read_response_denies_learned_revocation_before_transmission` — **retained mixed owner/protocol**: private authority setup, catalog/session gates and exact namespace/result matrix; shared filesystem denial is only partial replacement.
- `native_local_tokens_bound_api_actions_resource_kind_and_existing_socket_operations` — **retained mixed owner/protocol**: private authority setup, catalog/session gates and exact namespace/result matrix; shared filesystem denial is only partial replacement.
- `native_local_token_authorized_write_survives_revoked_response_and_next_operation_denial` — **retained mixed owner/protocol**: private authority setup, catalog/session gates and exact namespace/result matrix; shared filesystem denial is only partial replacement.
- `independent_idp_service_loss_denies_response_and_api_without_cached_fallback` — **retained independent fault case**: stop separately remote IdP during paused response and deny without cached fallback; not same Unified lifecycle.
- `native_catalog_create_response_denies_learned_user_deletion_without_rollback` — **retained mixed owner/protocol**: private authority setup, catalog/session gates and exact namespace/result matrix; shared filesystem denial is only partial replacement.
- `native_catalog_all_result_kinds_recheck_after_execution_and_retain_mutations` — **retained mixed owner/protocol**: private authority setup, catalog/session gates and exact namespace/result matrix; shared filesystem denial is only partial replacement.
- `generated_rust_and_built_javascript_use_actual_owner_routes` — **retained SDK qualification**: built JS and Rust clients against actual owner routers; executed by ticket05, not multiplied across process layouts.

### `crates/unified-server/src/node_bootstrap_interruption_tests.rs` — private unit / mixed (see exceptions)

- `bootstrap_current_boundaries_preserve_admitted_import_and_native_retry` — **retained private recovery**: durable transition/receipt/import checkpoints and orchestrator ordering cannot be induced by public process cancellation.
- `desktop_bootstrap_orchestration_completes_both_public_contexts_and_retries_offline` — **retained private recovery**: durable transition/receipt/import checkpoints and orchestrator ordering cannot be induced by public process cancellation.
- `production_durable_transitions_preserve_imports_and_verified_readiness_on_reopen` — **retained private recovery**: durable transition/receipt/import checkpoints and orchestrator ordering cannot be induced by public process cancellation.

### `crates/unified-server/src/node_join_bootstrap_tests.rs` — shared Node system / mixed private fixture

- `authorized_node_bootstrap_imports_both_contexts_and_reopens_offline` — **retained private cutover**: prepared owner bindings, local credential/receipt boundaries and offline cutover require private state.
- `owner_cutover_node_bootstrap_reopens_offline` — **retained private cutover**: prepared owner bindings, local credential/receipt boundaries and offline cutover require private state.

### `crates/unified-server/src/owner_composition_tests/create.rs` — deployment-specific system

- `production_independent_create_and_offline_reopen` — **retained deployment-specific**: independent owner protected capabilities, remote SQL/retained worker records and reset/recovery; not local-owner process parity.
- `production_independent_create_legacy_record_reset` — **retained deployment-specific**: independent owner protected capabilities, remote SQL/retained worker records and reset/recovery; not local-owner process parity.
- `independent_create_child` — **retained deployment-specific**: independent owner protected capabilities, remote SQL/retained worker records and reset/recovery; not local-owner process parity.

### `crates/unified-server/src/owner_composition_tests/join.rs` — deployment-specific system

- `production_independent_join_and_source_offline_reopen` — **retained deployment-specific**: remote-owner attachment, interrupted workers/legacy record/reset and recovery; not independently retained target history.
- `production_independent_join_legacy_record_reset` — **retained deployment-specific**: remote-owner attachment, interrupted workers/legacy record/reset and recovery; not independently retained target history.
- `independent_join_child` — **retained deployment-specific**: remote-owner attachment, interrupted workers/legacy record/reset and recovery; not independently retained target history.

### `crates/unified-server/src/owner_composition_tests/operation.rs` — deployment-specific system

- `production_independent_signer_free_operation_and_delayed_authority_reopen` — **retained deployment-specific**: signer-free remote consumers and delayed authority recovery, not Unified or fresh Join.
- `operation_child` — **retained deployment-specific**: signer-free remote consumers and delayed authority recovery, not Unified or fresh Join.

### `crates/unified-server/src/owner_composition_tests/permissions.rs` — deployment-specific system

- `independent_permission_production_management_https` — **retained deployment-specific**: production Management HTTPS binding and independently configured evaluator relationship.
- `independent_permission_management_https_child` — **retained deployment-specific**: production Management HTTPS binding and independently configured evaluator relationship.

### `crates/unified-server/src/owner_composition_tests/remote_setup.rs` — deployment-specific system

- `remote_create_join_reopen_and_device_reset_preserve_shared_installation` — **retained deployment-specific**: shared remote owner persistence and consumer reset must preserve remote Installation rows.
- `remote_setup_child` — **retained deployment-specific**: shared remote owner persistence and consumer reset must preserve remote Installation rows.

### `crates/unified-server/src/owner_composition_tests/transfer.rs` — deployment-specific system

- `stale_cleanup_cannot_clear_current_binding_and_exact_retry_stays_restricted` — **retained deployment-specific owner**: retirement/cleanup exact binding, stopped owner return and HTTPS transfer; shared Create/Join does not cover copy cleanup.
- `original_owner_return_keeps_retirement_pending_after_stopped_reopen` — **retained deployment-specific owner**: retirement/cleanup exact binding, stopped owner return and HTTPS transfer; shared Create/Join does not cover copy cleanup.
- `independent_transfer_production_https` — **retained deployment-specific owner**: retirement/cleanup exact binding, stopped owner return and HTTPS transfer; shared Create/Join does not cover copy cleanup.
- `independent_transfer_https_child` — **retained deployment-specific owner**: retirement/cleanup exact binding, stopped owner return and HTTPS transfer; shared Create/Join does not cover copy cleanup.

### `crates/unified-server/src/permission_http_tests.rs` — owner-only

- `normal_permission_http_administration` — **retained owner fixture; relocation deferred**: multiple seeded actors/Applications, confidential evaluator delegation and reserved Installation grants span injected owner interfaces; do not expose runtime authorizer or add server dependency.
- `normal_identity_and_infrastructure_lifecycle` — **retained owner fixture; relocation deferred**: multiple seeded actors/Applications, confidential evaluator delegation and reserved Installation grants span injected owner interfaces; do not expose runtime authorizer or add server dependency.
- `shared_identity_management_writes_change_idp_authorization` — **retained owner fixture; relocation deferred**: multiple seeded actors/Applications, confidential evaluator delegation and reserved Installation grants span injected owner interfaces; do not expose runtime authorizer or add server dependency.

### `crates/unified-server/src/policy_cleanup_http_tests.rs` — owner-only

- `real_policy_http_chain_retires_copies_reselects_generation_and_gates_transfer` — **retained mixed owner wiring**: real cross-owner cleanup/residency/transfer chain and exact generations are not replaced by shared ordinary resource reads.

### `crates/unified-server/src/remote_device_process_tests.rs` — deployment-specific system

- `remote_device_process_admission_and_consumer_reset_preserve_shared_rows` — **retained deployment-specific**: remote Device SQL process admission and consumer reset preserve owner rows; not local-owner Join history.
- `remote_device_child` — **retained deployment-specific**: remote Device SQL process admission and consumer reset preserve owner rows; not local-owner Join history.

### `crates/unified-server/src/remote_identity_process_tests.rs` — deployment-specific system

- `independent_processes_share_remote_identity_and_consumer_reset_preserves_rows` — **retained deployment-specific**: shared remote identity process persistence/reset; remote attachment is not independently retained target state.
- `remote_identity_child` — **retained deployment-specific**: shared remote identity process persistence/reset; remote attachment is not independently retained target state.

### `crates/unified-server/src/residency_http_tests.rs` — owner-only

- `authorized_whole_filesystem_residency_uses_public_storage_owner` — **retained owner/fault fixture; relocation deferred**: durable residency mode, exact-copy serialization, cleanup fences and paused owner failures use injected repository/authorizer gates.
- `public_residency_read_reports_durable_mode_after_native_reopen` — **retained owner/fault fixture; relocation deferred**: durable residency mode, exact-copy serialization, cleanup fences and paused owner failures use injected repository/authorizer gates.
- `overlapping_residency_puts_serialize_only_the_exact_copy_and_reopen_consistently` — **retained owner/fault fixture; relocation deferred**: durable residency mode, exact-copy serialization, cleanup fences and paused owner failures use injected repository/authorizer gates.
- `residency_denials_preserve_mode_and_cleanup_fences` — **retained owner/fault fixture; relocation deferred**: durable residency mode, exact-copy serialization, cleanup fences and paused owner failures use injected repository/authorizer gates.
- `residency_rechecks_learned_user_denial_before_owner_mutation` — **retained owner/fault fixture; relocation deferred**: durable residency mode, exact-copy serialization, cleanup fences and paused owner failures use injected repository/authorizer gates.
- `residency_rechecks_deleted_target_before_owner_mutation` — **retained owner/fault fixture; relocation deferred**: durable residency mode, exact-copy serialization, cleanup fences and paused owner failures use injected repository/authorizer gates.
- `residency_owner_write_failure_preserves_prior_policy_and_content` — **retained owner/fault fixture; relocation deferred**: durable residency mode, exact-copy serialization, cleanup fences and paused owner failures use injected repository/authorizer gates.

### `crates/unified-server/src/runtime.rs` — shared Node system / mixed private fixture

- `host_api_base_must_match_the_loopback_listener` — **retained Unified wiring/private**: host listener/owner handles, ALPN and injected authorization/membership wiring; external routes and shared process readiness are additional coverage.
- `builds_three_prefixed_services_with_one_endpoint` — **retained Unified wiring/private**: host listener/owner handles, ALPN and injected authorization/membership wiring; external routes and shared process readiness are additional coverage.
- `native_membership_restores_offline_and_learned_revocation_removes_admission` — **retained Unified wiring/private**: host listener/owner handles, ALPN and injected authorization/membership wiring; external routes and shared process readiness are additional coverage.

### `crates/unified-server/src/runtime_peer_refresh_tests.rs` — private unit / mixed (see exceptions)

- `peer_refresh_is_idle_and_coalesces_both_sources` — **retained private**: coalescing/change notification/retry scheduler and silent read-commit boundaries.
- `peer_refresh_retries_failures_without_another_commit` — **retained private**: coalescing/change notification/retry scheduler and silent read-commit boundaries.
- `peer_refresh_tracks_real_sql_commits_and_read_commits_are_silent` — **retained private**: coalescing/change notification/retry scheduler and silent read-commit boundaries.

### `crates/unified-server/src/security_candidate_tests.rs` — private unit / mixed (see exceptions)

- `marker3_revalidates_semantic_recovery_and_preserves_local_contender` — **retained protocol/storage fault**: corrupt/conflicted/overwritten candidates and authenticated admission fences require scoped history/owner contexts.
- `approved_endpoints_without_idp_signers_exchange_and_device_revocation_denies` — **retained protocol/storage fault**: corrupt/conflicted/overwritten candidates and authenticated admission fences require scoped history/owner contexts.
- `approved_devices_reject_conflicted_public_device_bindings` — **retained protocol/storage fault**: corrupt/conflicted/overwritten candidates and authenticated admission fences require scoped history/owner contexts.
- `marker3_persists_pending_conflicts_and_tombstones_without_trust` — **retained protocol/storage fault**: corrupt/conflicted/overwritten candidates and authenticated admission fences require scoped history/owner contexts.
- `marker3_rejects_correlated_rebinding_of_authenticated_session_peer` — **retained protocol/storage fault**: corrupt/conflicted/overwritten candidates and authenticated admission fences require scoped history/owner contexts.
- `management_rejects_overwritten_invalid_device_history_before_export_or_apply` — **retained protocol/storage fault**: corrupt/conflicted/overwritten candidates and authenticated admission fences require scoped history/owner contexts.
- `marker3_rejects_semantic_candidates_in_both_directions_and_roles` — **retained protocol/storage fault**: corrupt/conflicted/overwritten candidates and authenticated admission fences require scoped history/owner contexts.
- `marker3_rejects_semantic_candidates_in_known_row_deltas` — **retained protocol/storage fault**: corrupt/conflicted/overwritten candidates and authenticated admission fences require scoped history/owner contexts.

### `crates/unified-server/src/security_history_tests.rs` — private unit / mixed (see exceptions)

- `private_jwk_remains_in_a_losing_conflict_behind_a_valid_public_projection` — **retained protocol/storage fault**: private verifier/JWK/grant-hash exclusion including hidden history/defaults/recovery cannot be inferred from successful Join.
- `marker3_rejects_private_jwk_history_before_disclosing_to_unvalidated_peer` — **retained protocol/storage fault**: private verifier/JWK/grant-hash exclusion including hidden history/defaults/recovery cannot be inferred from successful Join.
- `marker3_rejects_unvalidated_incoming_private_jwk_history_before_apply` — **retained protocol/storage fault**: private verifier/JWK/grant-hash exclusion including hidden history/defaults/recovery cannot be inferred from successful Join.
- `marker3_rejects_private_jwk_in_an_unvalidated_full_state` — **retained protocol/storage fault**: private verifier/JWK/grant-hash exclusion including hidden history/defaults/recovery cannot be inferred from successful Join.
- `marker3_rejects_private_jwk_schema_defaults_before_disclosure` — **retained protocol/storage fault**: private verifier/JWK/grant-hash exclusion including hidden history/defaults/recovery cannot be inferred from successful Join.
- `marker3_revalidates_late_private_history_in_recovery_on_both_roles` — **retained protocol/storage fault**: private verifier/JWK/grant-hash exclusion including hidden history/defaults/recovery cannot be inferred from successful Join.
- `marker3_rejects_overwritten_raw_grant_hashes_in_both_directions_and_roles` — **retained protocol/storage fault**: private verifier/JWK/grant-hash exclusion including hidden history/defaults/recovery cannot be inferred from successful Join.
- `marker3_valid_public_key_and_hash_histories_remain_syncable` — **retained protocol/storage fault**: private verifier/JWK/grant-hash exclusion including hidden history/defaults/recovery cannot be inferred from successful Join.

### `crates/unified-server/src/security_sync_tests.rs` — private unit / mixed (see exceptions)

- `authenticated_equal_peers_exchange_scoped_history_without_local_verifiers` — **retained protocol owner**: authenticated scoped history/table manifests and verifier exclusion, not public setup completion.

### `crates/unified-server/src/setup_host_tests.rs` — private unit / mixed (see exceptions)

- `fresh_host_awaits_choice_without_installation_or_credentials_and_reopens_exactly` — **moved external**: same assertion body now in `crates/unified-server/tests/setup_host.rs`; old unit body/declaration removed.
- `interrupted_host_settings_are_not_fresh_generation_permission` — **moved external**: same assertion body now in `crates/unified-server/tests/setup_host.rs`; old unit body/declaration removed.
- `neutral_production_history_is_required_without_changing_active_host_metadata` — **retained private host/TLS**: neutral/retained metadata and private OS trust substitution/HTTPS lifecycle, not merely public Node setup.
- `explicit_configuration_classifies_absence_and_never_selects_an_issuer` — **retained private host/TLS**: neutral/retained metadata and private OS trust substitution/HTTPS lifecycle, not merely public Node setup.
- `setup_https_uses_isolated_native_roots_and_never_installs_real_trust` — **retained private host/TLS**: neutral/retained metadata and private OS trust substitution/HTTPS lifecycle, not merely public Node setup.
- `setup_https_child` — **retained private host/TLS**: neutral/retained metadata and private OS trust substitution/HTTPS lifecycle, not merely public Node setup.

### `crates/unified-server/src/setup_runtime_tests.rs` — Unified public integration

- `owner_routes_stay_mounted_when_runtime_becomes_available_or_unavailable` — **moved external**: same assertion body now in `crates/unified-server/tests/owner_routes.rs`; old unit body/declaration removed.

### `crates/unified-server/src/unsupported_bootstrap_tests.rs` — private unit / mixed (see exceptions)

- `unsupported_platform_rejects_all_bootstrap_constructors_before_creating_state` — **retained platform-specific private**: unsupported-platform refusal before state creation; not executed on Linux.



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

Ticket03 is [resolved](issues/03-shared-restart-and-join.md): post-integration
validation passed 16 Management tests, one provider test and all seven lifecycle
scenarios. An earlier full seven-scenario run and paired Join repeat also pass.
The report records earlier failures and external N0 discovery requirements; these
results do not certify discovery availability on offline/unreliable networks.

## Ticket05 final audit (integration 120f7953a)

### Completeness and remaining seams

The 155 baseline identifiers above were compared with **actual** test declarations
from `git ls-tree`/`git show 191962716:crates/unified-server/src/...`, including
path-mounted and subprocess-child modules, not merely counted from this document.
There are no missing baseline declarations. Twelve bodies moved to external
consumers (four to the IdP client owner); the other 143 identifiers retain bodies,
with the one ticket04 Join case renamed/narrowed as explicitly mapped. Platform and
CLI cfg gates explain differences from Linux default test-runner counts. Existing
external `composition` tests are additional to those 155. Nested
`owner_composition_tests/review_transfer_restart_tests.rs` and `reset.rs` are
helpers called by the retained transfer/reset scenarios, not missing test entries.

This is **not** a claim that all remaining Unified tests are private unit tests.
Pure public host/config/router cases moved externally. Remaining owner-oriented
permission/residency/copy-cleanup cases still use shared injected owner fixtures,
private authorizers or scheduling gates; their physical relocation is explicitly
deferred, not called completed. Extracting those fixtures into an owner-local
seam must not introduce server-to-server dependencies or production visibility.
The per-identifier reasons above and ticket04 report identify their non-replaced
assertions. Public overlapping slices are shared; mixed bodies remain enabled.

### Live reconciliation is applicable and retained, not silently waived

Spec stories 14–16 and testing decisions require preserving applicable recovery,
synchronization and learned-denial observations. `full_node_tests` remains that
coverage for **live post-Join** behavior:

- `two_completed_full_hosts_reconcile_consumption_and_revocation` consumes/revokes
  on the source **after** completed Join, reconnects IdP security histories, and
  requires target denial; local per-host replay/race assertions also remain.
- `two_completed_full_hosts_learn_restrictions_on_retained_sockets` privately grants
  reserved Installation-policy actions, changes opposite Device restrictions
  while partitioned, reconnects Device security histories, denies subsequent
  operations on retained sockets, restores access and verifies unchanged bytes.
- `two_completed_full_hosts_join_partition_reopen_and_learn_denial` retains
  independent private-secret/JWKS/authorization and partition/reopen observations.

The first two were actually executed by ticket05: **2 passed** with a 64 MiB test
stack. The default-stack attempt first aborted; see the report. The third was not
rerun. `Pair::reconnect` uses live runtime endpoint access and
`SecurityProtocol::synchronize` with owner databases and local member context.
There is no public actual-process trigger for that exact boundary. Do not add a
production control, read private process databases/keys or substitute initial Join
history for this behavior. Sharing these *specific* live cases remains a concrete
qualification/migration gap until a supported public seam exists; preservation
and focused execution here satisfy the bounded retained-private exception, not
cross-layout live reconciliation certification. Shared initial-history consumed/
revoked denial and local User deletion on an existing socket are distinct coverage.

### Actual raw HTTP audit

`tests/system/{create,authorization,recovery}.rs` has **no raw HTTP requests**.
Reqwest constructs bounded verified generated-client transports, certificates and
URLs only. Ordinary setup/login/Device/Management/Storage operations use generated
models/clients; WebSocket resources use existing typed native clients. Both layout
entries call the same scenario/assertion functions. Only concrete setup, addressing,
launch, lifecycle and documented stale-offer status differ.

| Remaining location | Actual disposition of raw calls |
| --- | --- |
| External `http_host.rs` | Five lexical `.send()` sites: absent/wrong capability (loop), hostile Origin, exact 202 Create acceptance, exact 200 completed retry, exact 401 wrong-password denial. Four `oneshot` sites verify OpenAPI bytes, method/route availability and CORS headers. Intentional exact wire/status cases; generated status/completed retry/health are used for ordinary observations. Ticket05 replaced the last ordinary raw CLI-health poll. |
| External `owner_routes.rs`, `composition.rs` | Three lexical `oneshot` sites each; exact supplied-router mounts, publish/withdraw statuses, unknown routes, metadata and prefix/body behavior. These test Unified routing with supplied routers, not fake full-Node qualification. |
| `api_join_tests.rs`, `https_bootstrap_tests.rs`, `hosted_setup_integration_tests.rs`, `setup_host_tests.rs` | Mixed exact wire/Origin/unknown-field/status/capability cases **and ordinary legacy operations** around private retained intent/TLS/admission/crash/readiness fences. Not all raw calls are wire exceptions. The common supported-operation slice is now shared/generated; remaining fixture migration is deferred with private assertions retained. |
| `full_node_tests.rs`, `full_node_probe.rs` | Ordinary legacy OAuth/Device/resource requests surrounding private grants, local grant races, runtime-native reconciliation and private credential inventories. Not generated-client-first qualification; retained mixed coverage, not a shared-operation fallback. No whole body is replaced. |
| `native_api_tests.rs` | Legacy catalog helpers plus runtime `openapi.json` exact document/export comparison in generated Rust/built JS qualification. Response-boundary gates remain private. Native WebSocket `.send(frame)` is not raw HTTP. |
| `permission_http_tests.rs`, `device_authority_tests.rs`, `residency_http_tests.rs`, `policy_cleanup_http_tests.rs` | Owner/mixed actor/evaluator/authority/residency/cleanup matrices, including normal supported raw requests and exact negative statuses. Deferred fixture/placement migration is explicit; private grants/repositories/gates remain. Shared generated denial covers only the named common slice. |
| `runtime.rs`, `setup_runtime_tests.rs`, `cli.rs`, `node_join_bootstrap_tests.rs` | Injected owner/ALPN/membership, private permission withdrawal/readiness, exact Origin and receipt/cutover fixtures. Raw normal calls still exist beside wire checks; retained for these non-replaced boundaries, not used by shared process scenarios. |
| `local_provisioning_tests.rs` | One raw token request verifies direct bundled Management/IdP registration/Device-owner wiring against the retained runtime. Not replaced by a mere successful process health check. |
| `remote_{identity,device}_process_tests.rs`, `owner_composition_tests/{create,join,operation,permissions,remote_setup,transfer,review_transfer_restart_tests,reset}.rs` | Independent-only remote-owner/HTTPS/consumer-reset/cleanup/recovery fixtures. Ordinary legacy HTTP plus exact transport/security assertions remain; remote shared SQL is not independently persisted target Join. Keep separate, not claimed as shared/generated coverage. |

Thus generated-client-first is established for **normal shared operations**, not
retroactively asserted for every historical private/deployment fixture. There is
no supported-operation handwritten workaround in the shared suite, no missing
supported endpoint found and no behavioral contract changed for the fixtures.

### Hooks, helpers and dependency deletion

Verified actual definitions and callers, rather than accepting the old hook list:

| Item | Why it still exists / deletion |
| --- | --- |
| `device_engine` / `full_node_test_devices` | Retained full-node Device policy/restriction and HTTPS host private checks. Test-only field stays private. |
| `full_node_test_identity` | `setup_runtime_tests::status_tracks_revocation_in_existing_runtime` needs exact reserved context-grant removal/restoration. |
| `full_node_test_server` | Full-node native admission/reconnect and HTTPS bootstrap endpoint checks. |
| `node_join_source_for_test` | HTTPS private retained-offer lifetime/release assertions. |
| `trace_bootstrap_http_stages` | Test-cfg setup runtime, HTTPS and status-revocation diagnostics; not an exported production hook. |
| `full_node_runtime_fixture`, `SecretStore`, `assert_offers_released`, owner reset/transfer helpers | Still called by enabled mixed/private cases above. No visibility widened and no deletion merely to suppress coverage. |
| Unified `rcgen`, `rustls`, `tokio-rustls` dev dependencies | **Deleted**: no Rust reference in any remaining Unified src/tests/examples after generated transport moved to IdP client. Lock drops exactly those three Unified edges; packages remain for real consumers, including IdP transport and shared WSS. |
| Unified duplicate dev `of-idp-client`, `of-storage-client` declarations | **Deleted**: existing normal dependencies are genuinely used by production verification and suffice for integration tests. No normal dependency removed. |

No newly unused runtime hook or test module was found. Reduction is real but
bounded: 12 internal bodies removed/moved, old transport module/fixture ownership
moved, 35 duplicate public Join assertions removed after replacement, three unused
direct edges and two duplicate declarations removed. Nine opt-in executions share
behavior, not nine separately maintained implementations. No unrelated helper,
native transport, JS qualification or production lifecycle was deleted.

Final commands, earlier failures and remaining limits:
[ticket05 report](ticket05-report.md). No CRAP, coverage or feature-powerset run.
