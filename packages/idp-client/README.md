# IdP Client

IdP Client is generated typescript client for the IdP OpenAPI specification. It is used to interact with the IdP API.

## Unified setup subpath

`@aicacia/idp-client/setup` is generated separately from the unified host's `/idp/setup/openapi.json`; it does not replace or extend the normal IdP owner inventory. From this package:

```sh
OPENAPI_SPEC_URL=https://node.example/idp/setup/openapi.json pnpm run openapi:setup
pnpm run build:esm
```

The existing generator validates the five implemented setup operations before replacing `src/setup`. Normal IdP regeneration preserves that subtree. Run generation commands serially; concurrent publication is not supported. Generated source must not be edited by hand.

Configure the SDK with the intended verified HTTPS **host origin**, not a service-prefixed base: generated operation paths already include `/idp`. Send the complete `Setup <code>` value as the API-key authorization value, keep it transient, and explicitly omit cookies, reject redirects and disable caching. A setup code is not an OAuth bearer token. Public status does not require it. `202` is acceptance, not completion; observe status. Intent lookup can return204 (unbound), and cancellation204 means a signal, not completed cleanup.

The app consumes generated request serializers through its existing checked HTTPS setup helper. Those helpers validate service targets and wire evidence and avoid exposing protected response/error data. The raw generated SDK is not a replacement for these checks. Independent-owner Create and Join are not advertised by this setup document.
