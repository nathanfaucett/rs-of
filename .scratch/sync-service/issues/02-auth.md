# 02: Auth — Valid Token + Selected Resource = Sync Authorized

**Status:** `ready-for-agent`
**Type:** `task`
**Blocked by:** 01

## Description

Implement `authorize_sync()` function and wire it into the WebSocket handler. Valid IdP token + resource selected for this app → connection accepted.

## What it delivers

- `AttachRequest` with valid JWT → `AttachResponse { accepted: true }`
- Invalid JWT → `AbortReason::Unauthorized`
- Resource not selected for this app → `AbortReason::Unauthorized`
- No sync session yet — just auth decision

## Scope

- `storage-server/src/sync.rs`:
  - `authorize_sync(idp_client, device_client, token, store_identity, local_endpoint)` function
  - Wire into `sync_handler`: on `AttachRequest`, call auth, respond
  - Pass `IdpClient`, `DeviceClient`, `EndpointId` in handler state

## Acceptance Criteria

- [ ] Valid token + selected resource → `AttachResponse.accepted == true`
- [ ] Expired token → `AbortReason::Unauthorized`
- [ ] Wrong audience/scope → `AbortReason::Unauthorized`
- [ ] Resource not selected for this app → `AbortReason::Unauthorized`
- [ ] Device Server unavailable → `AbortReason::BackendUnavailable`
- [ ] IdP unavailable → `AbortReason::BackendUnavailable`

## Notes

- Use `IdpClient::introspect()` for token validation
- Use `DeviceClient::selected_resources()` for selection check
- Match `store_identity.store_id` to selected resource `resource_id`
- Match token `application_id` to resource `application_id`
- Protocol types from ticket 01
