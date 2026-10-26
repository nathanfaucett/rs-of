# @of/sdk

Framework-independent authenticated browser Storage access, composing the existing OIDC and Storage clients. Workspace-only package; registry publication and SSR/native/confidential-client qualification are not included. The password manager is the working consumer.

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

Callback/restoration and Storage access use OIDC's `getVerifiedIdentity()`: obtain/renew an access token, use that exact token at the configured owner's userinfo endpoint, validate its nonempty subject, and bind the result to the unchanged credentials and discovered issuer. The OIDC result includes the verified token for the SDK's private Storage provider; it is never included in SDK snapshots or notifications. Storage exchanges only that verified token, without a second token acquisition. If credentials change during userinfo, OIDC discards the obsolete result (including obsolete errors) and retries current credentials; a stale read is not a terminal authentication failure. Authentication is **not** inferred from token presence, decoded JWT claims, or a no-code callback. This qualifies the trusted HTTPS discovery/token/userinfo workflow, not independent client-side ID-token signature verification. Trust the configured IdP and deploy browser-trusted TLS.

Renewal is demand-driven in the OIDC client; concurrent renewal is coalesced there. Same-identity renewal retains the exact session snapshot/context. A changed issuer/subject, missing credentials, invalid userinfo, or terminal authentication rejection retires the context and requires sign-in. Connectivity failures and resource/userinfo authorization denials do not indiscriminately erase an established session. Errors exposed by the SDK are bounded messages without provider bodies, callback parameters, or credential-bearing causes.

Local sign-out clears this client's credentials and pending transactions, invalidates retained Storage capabilities and connection requests, and notifies listeners **before** provider network work. `signout({ provider: true, redirectUri })` additionally delegates provider logout and returns `providerLogout: 'initiated' | 'failed' | 'not-requested'`. A provider failure cannot undo local termination. Initiating logout is not proof of provider-session termination or token revocation. Already-sent requests/writes are not canceled. No polling, background renewal, reconnect loop, cross-tab synchronization, or remote revocation promise is added.

## Persistence and redirect continuity

The SDK explicitly uses the OIDC client's `local` policy: access/refresh tokens and pending state-bound PKCE verifiers persist in localStorage across full-page authorization redirects and reloads. A new runtime can consume the pending transaction once. Cache namespaces bind the IdP base, OAuth Client, redirect URI, and registration metadata (including Application). One client clearing its state does not purge other clients. Unsafe old authority-only keys are ignored, not read or migrated; users must sign in again. Old plaintext legacy entries can be removed manually through browser site-data controls.

Browser storage must be available for this redirect policy. Credentials are plaintext to same-origin JavaScript and exposed to XSS, browser extensions, and local inspection; this is not a secure vault or an encrypted token store. No decrypted secrets or secret mutations are stored there. OIDC's separate memory mode does not persist credentials and cannot continue a full-page redirect in a new runtime. If browser storage removal fails, the instance and visible authentication still retire, but sign-out reports a bounded error requiring browser site-data cleanup; persisted deletion cannot be claimed. Local sign-out cannot establish deletion of data by the provider or defeat a compromised browser.

## Context-bound Storage and ownership

After callback/restoration establishes authentication, `sdk.storage()` returns a frozen, generation-bound `StorageAccess` with only `listFileSystems(signal?)`, `createFileSystem(name?, signal?)`, and `openSocket(filesystemId, signal?)`. It never exposes the mutable Storage client or a bearer-token provider. OIDC owns identity verification and demand-driven access-token renewal; the existing Storage client owns action-specific token exchange, resource HTTP requests, and WebSocket protocol. There is no SDK token cache. Explicit IdP/Storage prefixes and the configured OAuth Client ID are preserved. A catalog entry is not permission to open a filesystem; authorization remains server-side.

```ts
const storage = sdk.storage();
const resources = await storage.listFileSystems();
const created = await storage.createFileSystem('Example');
// Pass this capability directly to the existing collection adapter:
const collection = createStorageCollection({
  client: storage,
  userId: sdk.session.identity!.subject,
  applicationId: sdk.session.identity!.application,
  filesystemId: created.id,
  directory: 'records', schema: recordSchema, getKey: record => record.id,
});
await collection.preload();
// Application session subscriber: synchronously retire views and invoke cleanup.
// Dispose dependent live queries before their source collection.
await collection.cleanup();
```

`createStorageCollection` above is imported by the application from `@aicacia/storage-db-collection`, **not** by the SDK. The adapter consumes the Storage owner's structural connection seam (`socket` events, `request`, `close`); it retains all schema/path validation, optimistic settlement, reconciliation, deadlines, explicit Refresh, and collection cleanup logic. The app owns Filesystem ID selection, schemas, editor state, navigation, subscriptions, and official TanStack Svelte bindings. Neither package depends on the other, and there is no SDK collection registry or Svelte wrapper.

**Handoff:** the SDK owns an open attempt until `openSocket()` successfully resolves. Failed, canceled, or obsolete opens are closed by the Storage client/SDK before rejection, including a socket that finishes opening after retirement. Successful resolution transfers cleanup responsibility to the recipient; when the adapter awaits this capability, the adapter owns the adopted connection and closes it through its existing `cleanup()` (including disposal racing adoption). A direct caller must likewise close every successfully returned connection. Do not open a connection separately and then open another for the adapter. SDK disposal does not close an already transferred connection a second time; its synchronous notification lets the application dispose the adapter. Close is idempotent.

The connection guard exposes only `close`/`error` lifecycle event subscription, guarded `request`, and `close`, not raw `send` or the credential-bearing socket URL. Lifecycle events are fresh, payload-free events on an isolated EventTarget; listener receivers, targets, current targets, and event paths never expose the transport. Requests check the captured context before enqueueing, before sending queued work, and after completion; ended handles cannot initiate work or return late results as current success. Pending opens share a context abort signal, combined with the caller's optional signal. This is local cancellation, not proof that a remote create/write was canceled. List/create late results and failures cannot become successes in a replacement context. Already-sent mutations are never automatically replayed.

Renewal for the same verified identity retains the session and open collections. Filesystem-only switching is app/adapter cleanup, not an authentication reset. Retire the view/adapter synchronously on any generation or authenticated-state change; create a new SDK instance when changing configuration or after ending/disposal. Storage denials (including 401/403) and transient network failures reject the resource operation without automatically ending authentication. Terminal OIDC credential/identity failure still ends it. The password manager synchronously clears the authenticated view and offers an app-owned “Sign in again” link after termination; submitting sign-in replaces the ended SDK. Refresh reuses a healthy connection or explicitly reopens a failed one; it is not polling, push synchronization, or token revocation.

## Checks and limits

`pnpm --filter @of/sdk test` runs real OIDC workflows against controlled HTTP/browser responses. Tests cover guarded list/create/open, obsolete token exchanges/discovery/create/open completions, queued and late connection requests, one-owner/idempotent cleanup, caller cancellation, callback-generation replacement, Storage denial versus transient failure, fresh-runtime redirect continuity, reload, metadata/prefixes, invalid/no-op/substituted callbacks, client isolation, renewal/concurrency, identity changes, missing credentials, denial versus transient/terminal failures, local logout with failed provider discovery, unsubscribe, and disposal during discovery/callback/renewal. The password manager's mounted smoke additionally uses the real SDK, OIDC and Storage clients, collection adapter, and official TanStack binding for sign-in/callback, discovery/create/open, CRUD/Refresh/reload, filesystem switching, same-identity renewal retaining its editor/socket, pending-mutation context changes, and local sign-out teardown.

Controlled responses do not qualify a deployed provider, provider logout, cross-Node authorization convergence/replication, or production password-vault security. Follow the password manager's manual real-system qualification procedure with non-sensitive test records.
