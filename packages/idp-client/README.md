# IdP Client

Generated from the canonical `openapi/idp.json` contract using OpenAPI Generator 7.25.0. The owner package exports normal IdP operations and tagged `SetupApi`, `IndependentSetupApi`, `OwnerSetupApi`, and `IndependentJoinApi` modules. Host-coordinated setup remains at `/idp/setup/...`; it is not a fourth owner or SDK product.

Configure a complete verified HTTPS **IdP owner base**, including its mount prefix once. Send transient `Setup <code>` only to the intended setup operation; omit cookies, reject redirects and disable caching. Admission is not OAuth authorization. A 202 response accepts work, not completion; cancellation 204 signals cancellation, not finished cleanup.

From the repository root, export with `pnpm openapi:export`, generate with `pnpm openapi`, and validate the working tree with `pnpm openapi:check`. CI additionally uses `--committed`. See [the shared workflow](../../docs/api.md#generated-clients). Never hand-edit derived contracts or SDKs; run generation serially.
