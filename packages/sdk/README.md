# @of/sdk

Framework-independent browser application authentication, composing the existing OIDC client. Workspace-only package; registry publication and SSR/native/confidential-client qualification are not included. The password manager is the working consumer.

```ts
import { ApplicationSdk } from '@of/sdk';

const sdk = new ApplicationSdk({
  idpBaseUrl: 'https://node.example/idp',
  storageBaseUrl: 'https://files.example/storage',
  disableNativeRequests: true,
  client: {
    clientId: 'example-web',
    redirectUri: 'https://app.example/callback',
    responseType: 'code',
    registration: {
      application: { uri: 'example.application', name: 'Example' },
      clientId: 'example-web',
      redirectUris: ['https://app.example/callback'],
      scope: 'openid offline storage',
      tokenEndpointAuthMethod: 'none',
      clientType: 'public',
      grantTypes: ['authorization_code', 'refresh_token', 'urn:ietf:params:oauth:grant-type:token-exchange'],
      responseTypes: ['code'],
    },
  },
});
const unsubscribe = sdk.subscribe(session => {
  // Tokens are never delivered here. Retire context-bound views synchronously
  // when the session ends; navigation remains the application's responsibility.
});
await sdk.restore();                  // Ordinary load, not the callback route
await sdk.signin();                   // Existing OIDC authorization navigation
// On the configured callback route (including a fresh browser runtime):
await sdk.handleSigninCallback(new URL(window.location.href));
// Local-only, synchronous retirement before the returned promise settles:
await sdk.signout();
unsubscribe();
sdk.dispose();
```

`getAuthorizationUrl()` delegates to OIDC for applications/platforms that own navigation. `startRegistration()` delegates the existing registration navigation; neither is a new OAuth flow. Configuration is copied, immutable per instance, and requires distinct Application URI, OAuth Client ID, IdP base, Storage base, and callback URI. Project-owned generated IdP requests are the default; service prefixes are retained. The configured IdP's discovery supplies the canonical issuer, which can differ from its listener address.

## Identity and lifecycle

Sessions are frozen token-free snapshots with `status`, optional `identity` (issuer, User subject, Application URI, client ID), and instance-local `generation`. States are `initializing`, `unauthenticated`, `authenticated`, `ended`, and `disposed`. Subscription immediately delivers the current snapshot; unsubscribe stops future delivery. Disposal delivers the final retirement notification, clears subscribers, and is idempotent. Ended/disposed instances cannot start new authentication or token work; create a replacement instance for another sign-in/configuration.

Callback/restoration uses OIDC's `getVerifiedIdentity()`: obtain/renew an access token, use the configured owner's userinfo endpoint, validate its nonempty subject, and bind the result to the unchanged credentials and discovered issuer. Authentication is **not** inferred from token presence, decoded JWT claims, or a no-code callback. This qualifies the trusted HTTPS discovery/token/userinfo workflow, not independent client-side ID-token signature verification. Trust the configured IdP and deploy browser-trusted TLS.

Renewal is demand-driven in the OIDC client; concurrent renewal is coalesced there. Same-identity renewal retains the exact session snapshot/context. A changed issuer/subject, missing credentials, invalid userinfo, or terminal authentication rejection retires the context and requires sign-in. Connectivity failures and resource/userinfo authorization denials do not indiscriminately erase an established session. Errors exposed by the SDK are bounded messages without provider bodies, callback parameters, or credential-bearing causes.

Local sign-out clears this client's credentials and pending transactions, invalidates retained token providers, and notifies listeners **before** provider network work. `signout({ provider: true, redirectUri })` additionally delegates provider logout and returns `providerLogout: 'initiated' | 'failed' | 'not-requested'`. A provider failure cannot undo local termination. Initiating logout is not proof of provider-session termination or token revocation. Already-sent requests/writes are not canceled. No polling, background renewal, reconnect loop, cross-tab synchronization, or remote revocation promise is added.

## Persistence and redirect continuity

The SDK explicitly uses the OIDC client's `local` policy: access/refresh tokens and pending state-bound PKCE verifiers persist in localStorage across full-page authorization redirects and reloads. A new runtime can consume the pending transaction once. Cache namespaces bind the IdP base, OAuth Client, redirect URI, and registration metadata (including Application). One client clearing its state does not purge other clients. Unsafe old authority-only keys are ignored, not read or migrated; users must sign in again. Old plaintext legacy entries can be removed manually through browser site-data controls.

Browser storage must be available for this redirect policy. Credentials are plaintext to same-origin JavaScript and exposed to XSS, browser extensions, and local inspection; this is not a secure vault or an encrypted token store. No decrypted secrets or secret mutations are stored there. OIDC's separate memory mode does not persist credentials and cannot continue a full-page redirect in a new runtime. If browser storage removal fails, the instance and visible authentication still retire, but sign-out reports a bounded error requiring browser site-data cleanup; persisted deletion cannot be claimed. Local sign-out cannot establish deletion of data by the provider or defeat a compromised browser.

## Ticket 01 Storage handoff

For the existing collection-backed application during the auth slice, `tokenProvider()` captures the verified instance generation and revalidates identity/renews through OIDC before supplying the bearer token to the existing Storage client. Retained providers cannot follow a replacement SDK. The app still constructs its Storage client and owns collection/query disposal, filesystem selection, and editor state. Session retirement synchronously unmounts/disposes those views. No collection implementation was copied into the SDK.

Ticket 02 replaces this transitional composition with the small SDK-bound Storage capability and qualifies stale list/open results, retained connection guards, and connection handoff. Do not treat `tokenProvider()` as a guarantee that raw retained Storage sockets are lifecycle-guarded: the ticket 01 reference app closes them through the adapter on retirement. The collection adapter remains the sole owner of adopted sockets and record reconciliation.

## Checks and limits

`pnpm --filter @of/sdk test` runs real OIDC workflows against controlled HTTP/browser responses. Tests cover fresh-runtime redirect continuity, reload, metadata/prefixes, invalid/no-op/substituted callbacks, client isolation, renewal/concurrency, identity changes, missing credentials, denial versus transient/terminal failures, local logout with failed provider discovery, unsubscribe, and disposal during discovery/callback/renewal. The password manager's mounted smoke additionally uses the real SDK, OIDC and Storage clients, collection adapter, and official TanStack binding for CRUD/Refresh/reload and teardown.

Controlled responses do not qualify a deployed provider, provider logout, cross-Node authorization convergence/replication, or production password-vault security. Follow the password manager's manual real-system qualification procedure with non-sensitive test records.
