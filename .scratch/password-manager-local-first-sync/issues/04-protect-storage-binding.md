# 04: Preserve and protect the Storage binding across sessions

**What to build:** Retain the binding among the verified Installation, User/Application namespace, local database, and stable Resource ID. Reuse it across reloads and reconnects, and stop synchronization safely on sign-out, account switch, denial, or revocation without taking away local access.

**Blocked by:** 03 — Restore a remote-only database into a fresh browser

**Status:** ready-for-agent

Work status: resolved

Milestone: M2 — Single-Node encrypted synchronization

## Scope and non-goals

- Scope every binding to the verified Installation and namespace; never reuse it across Users or Installations.
- Keep attachment, cancellation, status, and retry in the sync adapter rather than vault CRUD. Detachment closes the Resource transport and stops scheduling without closing the local database or changing its contents.
- Retain the Resource-scoped transport interface across reconnects. Use one SQL Resource per WebSocket for this spec; the interface does not expose whether a future SDK implementation shares a socket.
- Clear unlocked keys on account switch and require confirmation before attaching to a different account.
- Do not treat a token as Device approval or bypass Storage Resource and Selection policy.

## Acceptance checks

- [ ] Reload, restart, token renewal, and reconnect reuse the same Resource without duplicates or repeated attachment prompts.
- [ ] A failed exchange exposes a recoverable status and retries against the same Resource without losing local edits.
- [ ] Sign-out and detachment stop the active sync session and prevent later upload until explicit reattachment.
- [ ] Switching Users or Installations clears unlocked keys and does not reuse the previous binding or upload local data automatically.
- [ ] Denied or revoked remote access stops authorized sync while the local database remains available.
- [ ] Browser tests cover both the locally created and remote-only attachment paths against a real backend.

## Current evidence and qualification limits

Merged in `3a3bc9ee`. The binding is local metadata, not replicated SQL history. The real non-seeded Chromium flow covered upload, restore, reload, denial, sign-out, and explicit reattachment. A verified receipt can recover a failed local binding save without another upload; local save does not grant sync permission. If browser storage cannot clear credentials, the current tab stops sync and reports incomplete sign-out. Reload can recover stale credentials, so restored permission alone cannot activate background sync.
