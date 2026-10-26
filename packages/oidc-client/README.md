# Aicacia OIDC Client

This package provides a lightweight OpenID Connect client with
support for dynamic client registration and both popup/redirect
sign‑in flows.

## Basic usage

```ts
import { OidcClient } from "@aicacia/oidc-client";

const client = new OidcClient({
  clientConfig: {
    authority: "https://provider.example.com",
    redirectUri: "https://app.example.com/callback",
    // optional: clientId may be omitted if the provider supports
    // dynamic registration
    clientId: "myclient",
  },
});

client.on("registered", (registration) => {
  console.log("dynamic registration client id", registration.client_id);
});

// redirect-based sign in
await client.signin();

// or open a popup
await client.signin({ popup: true });

// later, after handling the callback, load userinfo explicitly
const tokenResponse = await client.handleSigninCallback();
const identity = tokenResponse ? await client.getVerifiedIdentity() : null;
```

## Project-owned IdP APIs

Set `projectOwned: true` in `OidcClient` options when `clientConfig.authority` is the complete project IdP API base (including its configured prefix). Discovery, userinfo, token acquisition/refresh and dynamic registration then use the generated IdP SDK. Discovered credential recipients must match the generated operation on that owner base; redirects are refused. The supplied fetch implementation and configured request timeout still apply.

Both project applications select this mode. The default external-provider mode retains arbitrary OIDC endpoint layouts and native-protocol support; it is not a fallback for failed project-owned operations.

## Session storage

The default `storage: 'local'` persists tokens and state-bound PKCE transactions in browser localStorage, preserving full-page redirect and reload continuity. Namespaces bind authority service base, OAuth Client identity/configuration, redirect URI, and registration metadata (including Application). Legacy authority-only keys are ignored, not read or migrated; sign in again. Old plaintext entries can be removed manually through browser site-data controls.

`storage: 'memory'` writes no credentials or PKCE verifiers to browser storage and does not purge other clients' credentials. Reloading or constructing another client loses the session; it cannot continue a full-page redirect. `clearStoredTokenResponse()` clears only this client's credentials and pending transactions. Browser-persisted credentials are exposed to same-origin JavaScript/XSS, extensions, and local inspection; they are not an encrypted vault.

Sign-out clears local state before provider discovery/logout. Provider failure can still reject but cannot restore the local session. Local deletion is not remote revocation or proof of provider logout. Concurrent refresh is coalesced; callback/refresh results cannot persist credentials after local termination.

`getVerifiedIdentity()` obtains/renews an access token, loads userinfo, requires a nonempty subject and unchanged credentials, and returns the discovered canonical issuer, subject, and userinfo. This is a trusted discovery/token/userinfo workflow, not independent ID-token signature verification. Project-owned mode binds credential recipients to generated owner endpoints and refuses redirects. Issuer and service base are unambiguous HTTP(S) URLs, not necessarily the same URL.

Only state-bound authorization-code callbacks are accepted; no-code callbacks return `null`, and token-bearing callbacks are rejected instead of accepting substituted credentials. Sign-in no longer logs authorization URLs or options; userinfo errors do not retain provider response bodies. Applications should expose bounded error messages, never credential-bearing request/response objects.

## Features

- Automatically fetches and caches the provider's
  `/.well-known/openid-configuration`.
- Builds the authorization URL with sensible defaults and
  configuration-driven parameters.
- Handles dynamic client registration when the `clientId` is missing
  and the provider exposes a `registration_endpoint`. In browser
  environments the client will redirect the user to the registration
  endpoint so the request can be approved interactively; non-browser
  callers fall back to a simple POST.
- Emitted `error` events for unexpected issues.

See the `src` directory for more API details and tests.

## Breaking change

- The `registered` event now emits the full dynamic client registration response object.
- `setClientId` has been removed from `OidcClient`.
