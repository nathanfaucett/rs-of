# Spec: of/apps/app Admin UI Gaps

## Context

`of/apps/app` is the admin UI for managing the `of` IdP stack: **devices, users, clients (OAuth clients), and applications**. It is a SvelteKit + Tauri app consuming the `of` IdP, Management, and Device APIs via generated clients (`@ofsdk/idp-client`, `@ofsdk/management-client`, `@ofsdk/device-client`).

Current auth model: OIDC against the local `of` IdP (password grant for Tauri, Authorization Code + PKCE for web). No Auth0 integration.

---

## Current Capabilities (Working)

| Resource                 | Operations                                                                                          |
| ------------------------ | --------------------------------------------------------------------------------------------------- |
| **Applications**         | Full CRUD (list, create, view, edit, delete)                                                        |
| **Clients**              | Full CRUD per application; JWK viewing; redirect URIs, scopes, profiles, client types, auth methods |
| **Users**                | Read-only: list (paginated), detail (profile, roles across apps, consents), revoke consents         |
| **Devices**              | List, rename, revoke; QR pairing; 5s auto-refresh                                                   |
| **Roles & Permissions**  | Per-application: create/delete roles, create/delete permissions, assign/revoke role↔permission      |
| **Auth flows**           | Sign-in (password + auth code), callback, logout, native callback                                   |
| **Setup/onboarding**     | Bundled vs remote selection, HTTPS trust, health checks                                             |
| **Internationalization** | Paraglide i18n                                                                                      |

---

## Gaps for Stated Goal

> **Goal:** "management of devices, users, clients, and applications"

### 1. User Write Operations (Critical)

| Operation                        | API Status                                       | UI Status  |
| -------------------------------- | ------------------------------------------------ | ---------- |
| Create user                      | ❌ **MISSING** (needs 00-backend-user-create)    | ❌ Missing |
| Edit user profile                | ✅ `updateUser` exists                           | ❌ Missing |
| Delete user                      | ✅ `deleteUser` exists                           | ❌ Missing |
| Password reset (admin-initiated) | ✅ `resetPassword` exists                        | ❌ Missing |
| MFA enrollment/management        | ❌ **MISSING** (needs 00-backend-user-mfa-block) | ❌ Missing |
| Block/unblock user               | ❌ **MISSING** (needs 00-backend-user-mfa-block) | ❌ Missing |

**Seam:** `idpApi` — **VERIFIED**: `updateUser`, `deleteUser`, `resetPassword` exist
**MISSING**: `createUser`, MFA endpoints, block/unblock endpoints (require Rust IdP server work)

### 2. Device Enrollment (Critical)

| Operation                                                 | API Status                                       | UI Status  |
| --------------------------------------------------------- | ------------------------------------------------ | ---------- |
| Register new device (generate keypair, show QR)           | ✅ `enrollDevice` exists                         | ❌ Missing |
| Admin approve enrollment                                  | ❌ **MISSING** (needs 00-backend-device-approve) | ❌ Missing |
| Current device page only lists _already-enrolled_ devices | ✅ Works                                         | ✅ Works   |

**Seam:** `deviceApi` — **VERIFIED**: `enrollDevice({address, name, publicKey})` returns `{id, state, approvalCode?}`
**MISSING**: Explicit approve endpoint for admin-initiated enrollments (may need IndependentJoinApi)

### 3. Admin Dashboard (Nice to Have)

| Feature                         | Status     |
| ------------------------------- | ---------- |
| Overview stats (counts, health) | ❌ Missing |
| Quick actions                   | ❌ Missing |

**Seam:** New route `/` under `(auth)` layout aggregating existing API calls.

### 4. Confidential Client Support (Nice to Have)

Public-client dynamic registration works end-to-end (password-manager flow). Confidential clients can be created by admin but **`client_secret` is never displayed**.

| Gap                                                         | Location                                                         |
| ----------------------------------------------------------- | ---------------------------------------------------------------- |
| Show `client_secret` on creation (one-time modal with copy) | `applications/[applicationId]/+page.svelte` → `onCreateClient()` |
| "Reveal secret" button in client detail                     | `clients/[clientId]/+page.svelte`                                |
| "Regenerate secret" action                                  | `clients/[clientId]/+page.svelte` + IdP API                      |

**Seam:** `idpApi.register()` returns `clientSecret` (discarded); `idpApi.getRegister()` likely returns it (not exposed in UI).

---

## Non-Goals (Intentional)

- Active session management (tokens are immutable, not stored)
- Audit logs
- Auth0-specific features (Organizations, Actions, Rules, branding)
- Organizations/multi-tenancy
- Security policies (password policy, MFA policy, rate limiting, IP allow/deny)
- Branding/customization (login page, consent screen, emails)
- Backup/restore, monitoring, alerting

---

## Proposed Tickets (with blocking edges)

| #   | Title                                                   | Type | Priority | Blocked By            |
| --- | ------------------------------------------------------- | ---- | -------- | --------------------- |
| 00a | Backend: IdP createUser endpoint                        | task | Critical | —                     |
| 00b | Backend: IdP MFA and block/unblock endpoints            | task | Critical | —                     |
| 00c | Backend: Device approval for admin enrollment           | task | Critical | —                     |
| 00d | Backend: Client secret rotation endpoint                | task | Medium   | —                     |
| 01  | User create/edit/delete + password reset                | task | Critical | 00a, 00b              |
| 02  | Device enrollment flow                                  | task | Critical | 00c                   |
| 03  | Admin dashboard overview                                | task | Low      | —                     |
| 04  | Confidential client: show secret on creation            | task | Medium   | —                     |
| 05  | Confidential client: reveal/regenerate secret in detail | task | Medium   | 00d (regenerate only) |

---

## Seams — Verified

| Endpoint                              | Status            | Location                 |
| ------------------------------------- | ----------------- | ------------------------ |
| `idpApi.updateUser`                   | ✅ Exists         | `@ofsdk/idp-client`      |
| `idpApi.deleteUser`                   | ✅ Exists         | `@ofsdk/idp-client`      |
| `idpApi.resetPassword`                | ✅ Exists         | `@ofsdk/idp-client`      |
| `idpApi.createUser`                   | ❌ **MISSING**    | Needs Rust IdP server    |
| `idpApi` MFA endpoints                | ❌ **MISSING**    | Needs Rust IdP server    |
| `idpApi` block/unblock                | ❌ **MISSING**    | Needs Rust IdP server    |
| `deviceApi.enrollDevice`              | ✅ Exists         | `@ofsdk/device-client`   |
| `deviceApi` approve enrollment        | ❌ **MISSING**    | Needs Rust Device server |
| `idpApi.register` → `clientSecret`    | ✅ Returns secret | `@ofsdk/idp-client`      |
| `idpApi.getRegister` → `clientSecret` | ✅ Returns secret | `@ofsdk/idp-client`      |
| `idpApi` rotate secret                | ❌ **MISSING**    | Needs Rust IdP server    |

---

## Acceptance Criteria

### Ticket 01: User Write Operations

- Admin can create a user with email, username, password
- Admin can edit user profile fields
- Admin can delete a user (with confirmation)
- Admin can trigger password reset email
- Admin can enable/disable MFA for a user
- Admin can block/unblock a user

### Ticket 02: Device Enrollment

- "Add device" button on `/devices` page
- Generates keypair, shows QR code with device URI
- Admin approves → device appears in list, ready for sync

### Ticket 04: Show Secret on Creation

- After creating client in Application detail, modal shows `clientId` + `clientSecret` once
- Copy buttons for both
- Secret never shown again unless regenerated

### Ticket 05: Reveal/Regenerate in Detail

- "Reveal secret" button (requires confirmation) shows secret in modal
- "Regenerate secret" creates new secret, shows once, invalidates old

---

## Dependencies

- **01-user-write-ops** blocked by **00-backend-user-create** (create) and **00-backend-user-mfa-block** (MFA/block)
  - Edit/delete/reset can start immediately (APIs exist)
- **02-device-enrollment** blocked by **00-backend-device-approve** (approval step)
  - Enrollment + QR generation can start immediately (API exists)
- **04-client-secret-on-create**: **Ready** — no blockers
- **05-client-secret-reveal-regenerate**: Reveal **ready**; Regenerate blocked by **00-backend-client-secret-rotate**
- **03-admin-dashboard**: No blockers — aggregates existing APIs
- All tickets use existing UI patterns (Page, Card, Field, Alert, Table, notifications)

---

Status: needs-triage
