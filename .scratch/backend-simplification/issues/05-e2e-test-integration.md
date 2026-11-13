# Ticket 05: E2E Test Integration (Playwright + Vitest)

**Status: claimed**

Owner: `backend-simplification-e2e`, worktree `.scratch/worktrees/e2e`, base `64e44f11e`.

Test boundaries: real owner HTTP APIs (identity, Device, Storage); browser sign-in/session and vault operations. No backend substitutes. Scope excludes Rust; the resumed request explicitly includes the CI workflow.

Implementation complete; runtime acceptance remains blocked by ticket04/build qualification. See [exact validation and gaps](../e2e-validation.md) and [test commands](../../../e2e/README.md). Do not mark resolved until the real backend and browser flows pass.

**Blocked by:** 04

## What it delivers

Frontend E2E tests (password-manager and app) run against real unified-server backend. No mocks. Playwright globalSetup spawns server, tests hit real HTTP endpoints.

## Scope

### 1. Playwright Config (`of/e2e/playwright.config.ts` or app-level)

**Global Setup:**

```typescript
import { spawn } from "child_process";
import { waitFor } from "@playwright/test";

let backendProcess: ChildProcess;

export default async function globalSetup() {
  const dataDir = ".scratch/e2e";
  const port = 1355;
  const origin = "http://127.0.0.1:5173";

  backendProcess = spawn(
    "cargo",
    [
      "run",
      "-p",
      "unified-server",
      "--",
      "local",
      "--data-dir",
      dataDir,
      "--port",
      String(port),
      "--origin",
      origin,
    ],
    { stdio: "inherit" },
  );

  await waitForHealth(`http://127.0.0.1:${port}/idp/health`);
  await waitForHealth(`http://127.0.0.1:${port}/device/health`);
  await waitForHealth(`http://127.0.0.1:${port}/storage/health`);
  await waitForHealth(`http://127.0.0.1:${port}/management/health`);
}

export async function globalTeardown() {
  backendProcess?.kill();
}
```

### 2. Frontend Environment Variables

**Password-manager** (`.env.test` or vitest config):

```
PUBLIC_IDP_BASE_URL=http://127.0.0.1:1355/idp
PUBLIC_DEVICE_BASE_URL=http://127.0.0.1:1355/device
PUBLIC_STORAGE_BASE_URL=http://127.0.0.1:1355/storage
PUBLIC_OIDC_CLIENT_ID=password-manager
PUBLIC_OIDC_REDIRECT_URI=http://127.0.0.1:5174/callback
```

**App** (`.env.test`):

```
PUBLIC_IDP_BASE_URL=http://127.0.0.1:1355/idp
PUBLIC_DEVICE_BASE_URL=http://127.0.0.1:1355/device
PUBLIC_STORAGE_BASE_URL=http://127.0.0.1:1355/storage
PUBLIC_URL=http://127.0.0.1:5173
```

### 3. Vitest/Playwright Integration

- Update `vitest.config.ts` in password-manager and app to use Playwright provider
- Add `globalSetup` and `globalTeardown` pointing to the backend spawn script
- Tests use real HTTP clients (no MSW, no mocks)
- Optional: test data seeding via admin API in `beforeEach`

### 4. Delete Mock Backend Code

- Remove all mock handlers, MSW setup, fake API responses from frontend tests
- Tests now exercise actual API contracts

## Acceptance Criteria

- `pnpm test:e2e` (or equivalent) in password-manager spins up backend, runs tests, tears down
- Sign-in flow works: redirect to IdP → consent → redirect back → session established
- Vault CRUD operations work against real storage API
- Device enrollment/listing works against real device API
- App sign-in and device/storage access works
- No mock backend code remains in test files
- Test runs in CI (GitHub Actions) with `cargo run` available
