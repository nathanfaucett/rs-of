# IdP Client

IdP Client is generated typescript client for the IdP OpenAPI specification. It is used to interact with the IdP API.

## Unified setup subpath

`@aicacia/idp-client/setup` comes from the committed `openapi/idp-setup.json`, not a live server. It is separate from the normal owner inventory. From the repository root:

```sh
pnpm openapi:export
pnpm --filter @aicacia/idp-client run build:esm
pnpm openapi
pnpm openapi:check
```

See [the shared workflow](../../docs/api.md#generated-clients) for prerequisites, all ten documents, Rust SDKs and CI. Never hand-edit derived contracts or SDKs. All output is staged and compiled before publication; run generation serially.

Configure the SDK with the intended verified HTTPS **host origin**, not a service-prefixed base: generated operation paths already include `/idp`. Send the complete `Setup <code>` value as the API-key authorization value, keep it transient, and explicitly omit cookies, reject redirects and disable caching. A setup code is not an OAuth bearer token. Public status does not require it. `202` is acceptance, not completion; observe status. Intent lookup can return204 (unbound), and cancellation204 means a signal, not completed cleanup.

The app consumes generated request serializers through its existing checked HTTPS setup helper. Those helpers validate service targets and wire evidence and avoid exposing protected response/error data. The raw generated SDK is not a replacement for these checks. Independent-owner Create and Join are not advertised by this setup document.
