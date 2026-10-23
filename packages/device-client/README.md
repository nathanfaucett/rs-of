# Device client

Generated TypeScript client for the Device authority: Device lifecycle, endpoint trust, pairing and Node joining, restrictions, whole-resource selections, and replication policy.

Use a complete verified Device API base, including its mount prefix exactly once. Unified mounts this service at `/device`; Management's `/management` API is only for roles, permissions, and assignments.

```ts
import { Configuration, DefaultApi } from "@aicacia/device-client";

const devices = new DefaultApi(
  new Configuration({
    basePath: "https://node.example/device",
    accessToken: () => currentManagementAudienceToken(),
  }),
);
```

Device administration retains the `idp-management` actor audience and exact permissions. The HTTP base is not the OAuth audience. Storage replication callers require the configured service audience and exact existing `management.device.*` / `management.replication.*` scopes.

Regenerate both client families from `openapi/device.json` with `pnpm openapi:device`. Generated decoding does not authorize an operation or imply that Storage is ready.
