# Ticket 02 deleted coverage mapping

The host is a composer, not another owner. This inventory distinguishes retained external behavior from deliberately deleted representation/obsolete-lifecycle assertions. Owning-seam references identify existing tests, not a blanket assertion that every broader suite passed this turn; see ticket02-validation.md for exact execution results. Ticket 03 owns cross-layout qualification; ticket 04 owns full desktop consumer migration.

## `apps/app/src-tauri/src/native_provider_lifecycle_tests.rs`

Intentionally remove the desktop duplicate of the deleted hosted lifecycle. Owner-local Create/Join/reopen is exercised by unified-server/tests/live_reconciliation.rs; native prerequisite/reset refusal remains in desktop_lifecycle_tests.rs and setup_reset_tests.rs. The old desktop aggregate status/login contract and its provider fixture are obsolete; full desktop onboarding qualification belongs to ticket 04, not a claim of equivalent GUI coverage here.

Removed test entries:
- `native_shared_create_reopen_and_provider_safe_reset`
- `native_shared_create_child`

## `crates/unified-server/src/api_join_tests.rs`

Delete the old hosted-Join fixture and its timing-logger self-test: their consumer and host journal are deleted. Join approval/import belongs to device-server/idp-server independent_join and node_join contract suites; composed Join/reopen is exercised by live_reconciliation.rs. Intentionally drop logger-format coverage with the deleted logger.

Removed test entries:
- `categorical_timing_logger_emits_common_clock_without_humantime`

## `crates/unified-server/src/bootstrap_config_tests.rs`

Intentionally drop logical-issuer generation and same-port/distinct-issuer semantics. Canonical Create issuer is the exact IdP base; Join retains its authenticated source issuer. live_reconciliation.rs checks both, rather than reproducing the old contradictory issuer behavior.

Removed test entries:
- `independent_installations_on_the_same_local_port_have_distinct_issuers`

## `crates/unified-server/src/endpoint_provider_tests.rs`

Intentionally drop every host endpoint-reference/journal/provider-attempt/reset-format assertion: those representations and their implementation are deleted, not migrated. Endpoint identity belongs to the existing Device owner (device-service security_protocol_contract_tests.rs and security_candidate_tests.rs); owner retained-state refusal/reset lives in tests/owner_composition_tests/create.rs, join.rs and reset.rs. Endpoint stability after composed Create restart is checked by live_reconciliation.rs. Learned revocation belongs to owner protocol tests, not a second host implementation.

Removed test entries:
- `create_attempt_is_durable_before_provider_work_and_uncertainty_never_retries_creation`
- `failed_creation_readback_can_only_reopen_the_exact_written_identity`
- `ordinary_reopen_refuses_missing_corrupt_mismatched_and_unavailable_material_unchanged`
- `join_endpoint_is_stable_and_missing_attempted_material_cannot_be_replaced`
- `reset_retains_exact_endpoint_reference_through_deletion_failure_and_continuation`
- `unsupported_raw_endpoint_layout_is_refused_without_provider_work_or_replacement`
- `interrupted_attempt_before_provider_creation_refuses_and_preserves_reference`
- `iroh_rejects_connections_after_peer_revocation`
- `operation_without_endpoint_reference_field_is_refused_intact_without_provider_work`
- `reset_refuses_lost_create_directories_without_losing_sql_or_provider_references`
- `join_without_endpoint_reference_field_is_refused_without_sql_or_provider_work`

## `crates/unified-server/src/hosted_setup_integration_tests.rs`

Intentionally drop assertions about private host intent and partial hosted credential/reference recovery, since the host has no such state. Exact owner admission and missing/corrupt retained evidence are covered by api independent_setup_tests.rs/setup_admission_tests.rs. Composed login and retained Create/Join reopen are in live_reconciliation.rs. Child entries are fixtures, not extra behaviors.

Removed test entries:
- `retired_join_reopen_refuses_mismatched_or_missing_intent_without_owner_mutation`
- `partial_credentials_reopen_preserves_recovery_for_create_and_join`
- `production_api_create_authentication_and_offline_reopen`
- `api_create_child`

## `crates/unified-server/src/http_server_tests.rs`

Intentionally drop plaintext-host setup origin semantics: the replacement uses the single local HTTPS authority. Origin/admission guarding is owned by api independent_setup_tests.rs; fixed loopback TLS identity/refusal is owned by api local_tls.rs tests.

Removed test entries:
- `http_setup_origin_is_exact_and_loopback_only`

## `crates/unified-server/src/https_bootstrap_tests.rs`

Composed HTTPS Create/Join, exact/conflicting admission, per-owner retirement, pinned trust and retained/source-offline reopen are exercised by live_reconciliation.rs. Admission cancellation/retained-operation refusal are owned by api independent_setup_tests.rs/setup_admission_tests.rs; TLS address rejection is owned by api local_tls.rs. Intentionally drop host-wide atomic retirement, aggregate revocation/status, hosted Join cancellation state, and hosted journal interruption formats: there is no corresponding host state. Current owner readiness remains owner-local. Isolated child entries are fixtures.

Removed test entries:
- `production_https_create_join_and_offline_reopen_with_isolated_native_roots`
- `hosted_join_cancellation_exact_retry_and_offline_reopen`
- `production_api_join_interruption_preserves_private_host_intent`
- `production_api_join_completion_and_offline_reopen`
- `production_setup_status_tracks_current_local_revocation`
- `production_api_join_atomic_retirement_leftover_code_offline_reopen`
- `production_https_bootstrap_child`
- `unsupported_tls_address_refuses_before_any_retained_material`

## `crates/unified-server/src/local_provisioning_tests.rs`

Intentionally drop all LocalProvisioner/host-journal/host-metadata/credential-guard/old relationship-count/concrete-host-normalization assertions: these objects no longer exist and must not be recreated just to retain their tests. Password-input safety and signer/provider-attempt behavior remain owned by idp-server local_provisioning and node_provisioning tests; admission exactness/refusal is owned by api independent_setup_tests.rs/setup_admission_tests.rs. Composed login/reopen is in live_reconciliation.rs. Intentionally remove the stopped-host filesystem recovery fixture and its byte-diagnostic helper: deployment filesystem/restart qualification is ticket 03, while offs in-process integrity/resource/retirement suites remain unchanged. No claim that the not-yet-migrated deployment suite currently passes.

Removed test entries:
- `endpoint_private_material_is_provider_owned_and_stable_on_reopen`
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
- `join_missing_or_corrupt_host_metadata_never_replaces_endpoint_or_allocations`
- `local_provisioning_creates_exact_host_and_reopens_offline`
- `local_provisioning_continues_retained_attempt_and_refuses_changed_identity`
- `local_provisioning_lost_journal_refuses_even_with_ready_owner_state`
- `local_provisioning_lost_management_after_late_failure_denies_without_recreation`
- `local_provisioning_lost_storage_after_runtime_initialization_denies_without_recreation`
- `local_provisioning_missing_attempted_signer_denies_without_replacement`
- `local_provisioning_reuses_pre_enrollment_signer_after_readback_interruption`
- `local_provisioning_rejects_fifo_password_before_opening_it`
- `local_provisioning_rejects_unprotected_input_and_missing_or_conflicting_intent`
- `concrete_host_settings_preserve_the_durable_intent_when_normalized`
- `retained_create_sql_keeps_owner_evidence_and_exact_retry_checks`
- `join_choices_have_one_canonical_sql_owner_for_standalone_and_hosted_operations`

## `crates/unified-server/src/runtime_peer_refresh_tests.rs`

Intentionally drop the coalescing/retry/real-SQL-commit scheduling tests with the deleted UnifiedRuntime peer-refresh scheduler. Device owner runtime/security history now owns networking admission and refresh; device-service security_outbound_tests.rs tests learned grants/revocation. Do not preserve a second host scheduler for test parity.

Removed test entries:
- `peer_refresh_is_idle_and_coalesces_both_sources`
- `peer_refresh_retries_failures_without_another_commit`
- `peer_refresh_tracks_real_sql_commits_and_read_commits_are_silent`

## `crates/unified-server/src/setup_host_tests.rs`

Owner teardown/lock-release and queued request shutdown are owned by api setup_server_teardown_tests.rs. Owner retained evidence/refusal belongs to api independent_setup_tests.rs. Pinned composed HTTPS is exercised by live_reconciliation.rs; owner trust-install cancellation is tested by api setup_install_tests.rs. Intentionally drop host metadata/history selection and aggregate authority generation assertions; no host metadata exists.

Removed test entries:
- `shutdown_joins_router_teardown_before_releasing_host_and_admission`
- `shutdown_rejects_queued_request_before_joining_router_teardown`
- `neutral_production_history_is_required_without_changing_active_host_metadata`
- `explicit_configuration_classifies_absence_and_never_selects_an_issuer`
- `setup_https_uses_isolated_native_roots_and_never_installs_real_trust`
- `setup_https_child`

## `crates/unified-server/src/setup_runtime_tests.rs`

Helper assertions for aggregate current revocation/readiness are intentionally removed: there is no aggregate status or SetupRuntime. Current owner readiness/refusal is tested at api independent_setup_tests.rs and owner contract seams; revocation authorization remains with owner service/protocol tests.

This file supplies assertion helpers for the old host fixtures rather than standalone test entries. Their host aggregate-state assertions are dropped for the reason above.

## `crates/unified-server/src/unsupported_bootstrap_tests.rs`

Intentionally drop unsupported-platform calls to deleted BootstrapConfig constructors. No constructors or compatibility exports remain. This change is validated on Linux only; no cross-platform runtime qualification is claimed.

Removed test entries:
- `unsupported_platform_rejects_all_bootstrap_constructors_before_creating_state`

## `crates/unified-server/tests/bootstrap_config.rs`

Intentionally remove BootstrapConfig public-settings normalization, custom/fabricated issuer, HTTP/proxy fallback and stored bootstrap-choice assertions with that API. Exact canonical owner bases/Create issuer and authenticated Join peer/issuer are governed by api independent_operation and independent_setup_tests.rs; live_reconciliation.rs verifies the composed issuer/retained operations. Unsupported previous retained layout is refused intact by composition.rs. Old distinct logical issuer semantics are intentionally not preserved.

Removed test entries:
- `ordinary_create_derives_and_retains_all_public_host_settings`
- `automatic_neutral_create_selects_public_api_issuer`
- `proxy_create_uses_public_api_issuer_not_internal_listener`
- `explicit_custom_issuer_survives_neutral_setup_selection_and_reopen`
- `empty_explicit_issuer_retains_public_api_fallback_and_accepts_equivalent_config`
- `whitespace_explicit_issuer_is_rejected_without_writes`
- `explicit_issuer_conflicts_are_refused_without_changing_identity_or_database`
- `explicit_join_preserves_authenticated_peer_issuer_not_configured_create_issuer`
- `legacy_fabricated_issuer_is_rejected_without_rewriting_identity_or_database`
- `join_settings_require_peer_evidence_not_an_invented_installation`

## `crates/unified-server/tests/full_node.rs`

Intentionally remove repeated host-owned two-Node partition/refresh-consumption/revocation/restriction scenarios rather than porting their internal host fixture. Owner security_candidate/security_outbound/protocol tests and storage-server execution_authorization/database_execution/filesystem_execution tests retain domain denial coverage. Deployment-wide two-Node/retained-resource qualification belongs to ticket 03. Those larger composed scenarios are intentionally not claimed as rerun by the smaller live test.

Removed test entries:
- `two_completed_full_hosts_join_partition_reopen_and_learn_denial`
- `two_completed_full_hosts_reconcile_consumption_and_revocation`
- `two_completed_full_hosts_learn_restrictions_on_retained_sockets`

## `crates/unified-server/tests/host_sql.rs`

Exact/conflicting retry, owner retirement, public login and retained restart are exercised through live_reconciliation.rs and api setup_admission_tests.rs. Intentionally drop completed-host password retry/aggregate retirement semantics: capability lifetime and exact operation now belong to each owner.

Removed test entries:
- `create_retires_atomically_reopens_and_authenticates_completed_retries`

## `crates/unified-server/tests/host_sql_interruption.rs`

Owner concurrent exact/conflicting admission, cancellation and retained refusal are owned by api independent_setup_tests.rs/setup_admission_tests.rs; production owner interruption is exercised by tests/owner_composition_tests. Intentionally drop host SQL allocation/journal consistency assertions with the deleted HostStore.

Removed test entries:
- `admitted_create_survives_concurrent_retries_cancellation_and_restart_without_reallocation`

## `crates/unified-server/tests/host_sql_transactions.rs`

Intentionally drop all host SQL transaction/schema/aggregate completion atomicity assertions: there is no host SQL schema, operation or retirement transaction. api setup_admission_tests.rs retains durable per-owner binding/retirement/failure-poisoning coverage. No equivalence to a cross-owner transaction is claimed or desired.

Removed test entries:
- `failed_completion_transaction_preserves_authority_and_retirement_survives_cleanup_failure`
- `join_completion_and_retirement_are_one_transaction`
- `unsupported_host_schema_refuses_without_replacement_authority`

## `crates/unified-server/tests/http_host.rs`

Fresh owner OpenAPI publication is owned by api independent_setup_tests.rs; composed routing is in composition.rs. Intentionally drop no-certificate/plain-HTTP-host setup, now replaced by one pinned HTTPS endpoint. live_reconciliation.rs exercises that endpoint; child/provider helpers disappear with the old fixture.

Removed test entries:
- `fresh_host_publishes_complete_openapi_without_provisioning`
- `http_setup_create_and_reopen_without_certificates`
- `http_setup_child`

## `crates/unified-server/tests/owner_routes.rs`

Composed map/prefix/normal-route and absent old/setup paths belong to composition.rs. Owner normal-router publication/CORS belongs to api independent_setup_tests.rs; live_reconciliation.rs observes actual owner service availability before and after retained restart. No second host runtime availability model is retained.

Removed test entries:
- `owner_routes_stay_mounted_when_runtime_becomes_available_or_unavailable`

## `crates/unified-server/tests/setup_host.rs`

Fresh owner admission and retained missing/corrupt/unsafe state refusal belong to api independent_setup_tests.rs/setup_admission_tests.rs. composition.rs tests previous-host and orphaned-endpoint refusal without reprovisioning. Intentionally drop host database initialization, interrupted host settings and host schema assertions, because that authority store is removed.

Removed test entries:
- `fresh_host_awaits_choice_without_installation_or_credentials_and_reopens_exactly`
- `interrupted_host_settings_are_not_fresh_generation_permission`
- `awaiting_choice_missing_database_refuses_without_recreating_authority`
- `empty_retained_database_is_not_initialized_or_repaired`
- `corrupt_database_and_legacy_layout_are_refused_intact_without_admission`
- `interrupted_code_creation_never_mints_replacement_authority`

## `crates/unified-server/tests/support/full_node.rs`

Delete the obsolete full-node helper/provider fixture with its consumers. Intentionally drop tests of that fixture itself (private password fixture layout and mock provider modifiers); production owner provider behavior is not inferred from helper tests. New live composition uses isolated temporary roots and the mock provider.

Removed test entries:
- `fixture_retains_owner_only_root_and_password`
- `node_stores_ignore_modifiers_without_crossing_namespaces`

## `tests/unified_identity_composition.rs`

Remove the stale UnifiedEndpoint/shared-identity fixture and its broken imports. Identity/permission writes and authorization are owned by tests/owner_boundaries permission_http_tests/device_authority_tests and owner_composition_tests/permissions.rs. Intentionally do not keep a host-owned shared-state integration fixture after replacing the lifecycle.

Removed test entries:
- `shared_identity_management_writes_change_idp_authorization`

## Tests removed from rewritten implementation files

- `src/config.rs`: `explicit_bootstrap_retains_concrete_server_options_and_public_references`, `default_unified_config_loads_without_relationship_secrets`, `default_standalone_configs_load_without_relationship_secrets`, `default_unified_config_supports_fresh_explicit_bootstrap_and_retained_issuer`, and `bundled_config_needs_no_relationship_secrets_and_refuses_removed_configuration` are intentionally dropped with BootstrapConfig/CredentialBootstrap and the old freshness checker. Shared file safety is exercised by api setup_admission_tests.rs; retained previous layouts are refused by composition.rs. Owner configuration remains the actual owner AppConfig, not a host-normalized mirror. The standalone sample-loading duplicate is intentionally dropped from the host crate; current sample/document migration belongs to ticket 04.
- `src/runtime.rs`: `host_api_base_must_match_the_loopback_listener`, `builds_three_prefixed_services_with_one_endpoint`, and `native_membership_restores_offline_and_learned_revocation_removes_admission` lose their UnifiedRuntime fixture. Composition is covered by composition.rs; fixed TLS addresses by api local_tls tests; offline owner reopening by live_reconciliation.rs. Intentionally drop host-owned native membership scheduling/assertions in favor of owner protocol/security seams. The old runtime helper functions have no independent coverage obligation after their callers are deleted.
- `tests/composition.rs`: `composition_exposes_only_routes_supplied_by_the_four_services` and `owner_service_builders_supply_hosted_features_to_composition` are replaced by the single prefix/path-map test; owner contract-export inventory assertions are not host responsibility. Canonical export remains in the unchanged export-openapi example; API/SDK inventory and regeneration qualification remain with owner contract suites and ticket 04. The replacement is exactly one composition test.
