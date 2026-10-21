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
const { url, window } = await client.signin({ popup: true });

// later, after handling the callback, load userinfo explicitly
const tokenResponse = await client.handleSigninCallback();
const userInfo = tokenResponse ? await client.getUserInfo() : null;
```

## Project-owned IdP APIs

Set `projectOwned: true` in `OidcClient` options when `clientConfig.authority` is the complete project IdP API base (including its configured prefix). Discovery, userinfo, token acquisition/refresh and dynamic registration then use the generated IdP SDK. Discovered credential recipients must match the generated operation on that owner base; redirects are refused. The supplied fetch implementation and configured request timeout still apply.

Both project applications select this mode. The default external-provider mode retains arbitrary OIDC endpoint layouts and native-protocol support; it is not a fallback for failed project-owned operations.

## Session storage

Set `storage: 'memory'` in the client options for an in-memory session. Tokens and PKCE verifiers are not written to browser storage, and existing `oidc.token:` and `oidc.pkce:` caches for all authorities in the same browser storage are removed when the client is constructed or cleared. Cache-removal failures are surfaced, not silently ignored. `clearStoredTokenResponse()` clears the session. Reloading or constructing a new client requires signing in again; redirect flows that replace the client instance cannot retain a memory-only PKCE verifier.

The default remains `storage: 'local'` for existing browser clients. The desktop application explicitly selects memory-only storage and clears its session on signout and reset.

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
