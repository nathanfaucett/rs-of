# 03: OfflineStatus → toast notifications

**What to build:** Convert persistent OfflineStatus banner to ephemeral toast notifications via existing `Notifications` system. Suppressed in `dev` mode.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] `OfflineStatus.svelte` removes all banner markup (`<aside>`, status/error paragraphs, button)
- [ ] On `status` change (offline ready, error) → `notifications.show({ message: status, tone: 'info' })`
- [ ] On `storageError` change → `notifications.show({ message: storageError, tone: 'warning' })`
- [ ] On `online` change (offline → online) → `notifications.show({ message: 'Back online', tone: 'success' })`
- [ ] Import `dev` from `$app/environment`; no-op all toasts when `dev === true`
- [ ] No DOM rendered by OfflineStatus (component becomes headless)
- [ ] Unit test: no banner DOM; notifications called on status/error/online changes; suppressed in dev
- [ ] Integration test: simulate offline → toast appears; dismiss → gone; no persistent banner
