# @ofsdk/sync-sdk

For a caller-owned SQL `Database`, use `createSqlResourceSync` from
`@ofsdk/sync-sdk/browser` (also exported at the package root). This lifecycle
opens no local database and does not start until you call `sync()`.

```ts
import { createSqlResourceSync, SyncError } from "@ofsdk/sync-sdk/browser";

const upload = createSqlResourceSync(database, identity, {
  mode: "first-upload",
  storageApiUrl,
  tokenProvider: getSyncTokenForConfirmedAccount,
});

try {
  await upload.sync();
  // Storage confirmed publication. The app can now record the established binding.
} catch (error) {
  if (error instanceof SyncError && error.code === "destination-has-history") {
    // Stop. The Resource may have independent history or a prior unconfirmed upload.
  }
  throw error;
} finally {
  await upload.close(); // The app still owns and can use database.
}
```

- Confirm the destination before first upload. `identity.storeId` is the remote
  SQL Resource ID, not the local database name. Use the verified Storage URL
  and a token provider scoped to the confirmed User and Application. The
  lifecycle copies the identity, URL, provider reference, and mode. It obtains
  one token per attempt; do not use a provider that follows account switches.
- An attach response or completed SQL exchange is not proof of publication.
  If the receipt is lost, publication is unknown. A retry uses the same Resource
  and guarded mode. A history rejection is not proof that a previous upload
  succeeded. There is no automatic fallback or retry.
- Use `mode: "established"` for a separately verified binding. It uses normal
  bidirectional exchange and must not be used to bypass a first-upload rejection.
- Concurrent calls to `sync()` on one lifecycle share one exchange. `close()`
  cancels and waits for it without closing the database. Close on sign-out or
  account change. The lifecycle does not register Resources, grant Selection,
  persist bindings, or schedule sync on local writes.

The legacy `BrowserSyncSDK` and `createEstablishedSqlHandle` remain available.
