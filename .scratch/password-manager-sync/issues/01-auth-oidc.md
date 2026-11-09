# Ticket 01: OIDC Authentication Integration

## Type: task
## Status: open

### Description
Add OIDC authentication to password-manager using `@ofsdk/oidc-client` and `@ofsdk/idp-client`. This enables the app to obtain access tokens for Storage Service API calls.

### Acceptance Criteria
- [ ] Add `@ofsdk/oidc-client` and `@ofsdk/idp-client` to package.json dependencies
- [ ] Create `src/lib/auth/oidc.ts` with `OidcAuth` class:
  - `initialize(config)` — discovers IdP config, sets up client
  - `login()` — redirects to IdP authorization endpoint
  - `handleCallback()` — processes redirect, exchanges code for tokens
  - `getAccessToken(): Promise<string>` — returns valid access token (refreshes if needed)
  - `logout()` — clears local session, optionally calls IdP end-session
  - `getUserInfo()` — returns user claims (sub, email, name)
- [ ] Create `src/lib/auth/auth.svelte.ts` with Svelte 5 reactive state:
  - `user` — current user info or null
  - `isAuthenticated` — boolean
  - `isLoading` — during init/callback
  - `login()`, `logout()` actions
- [ ] Integrate with app layout: show login page when not authenticated, vault UI when authenticated
- [ ] Store tokens in memory (not localStorage) for security
- [ ] Auto-refresh token before expiry (use `expires_in` from token response)

### Implementation Notes
- Reference `of/apps/app/src/lib/common/state/idpClient.svelte.ts` for patterns
- IdP base URL from env: `VITE_IDP_BASE_URL` (default `https://idp.localhost:1355`)
- Client ID: `password-manager` (registered in IdP)
- Redirect URI: `https://password-manager.localhost:1355/callback`
- Scopes: `openid profile email storage:read storage:write`

### Dependencies
- None (first ticket)

### Testing
- Unit: `OidcAuth` token refresh logic, callback handling
- Integration: Full login flow against local IdP (Keycloak)
- E2E: Playwright test — login → verify token → access Storage Service
