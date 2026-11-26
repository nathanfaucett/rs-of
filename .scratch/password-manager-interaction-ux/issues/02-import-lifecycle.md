# 02: Repair Import Cancel and completion states

**What to build:** Import Cancel returns to a usable file picker on the same Settings visit. A successful import retires its committed preview, prevents another commit from that preview, and offers an explicit clean new-file action.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Work status:** resolved

## Scope

- Reset only active file, preview, confirmation, and transient error state on Cancel.
- Allow a corrected file after invalid validation without leaving Settings.
- Retire completed previews and provide clear completion and new-file states with useful focus placement.
- Preserve complete-batch commits, duplicate confirmation, new-local-identity behavior, and uncertain-write recovery guidance.

## Non-goals

- No skip, merge, overwrite, automatic duplicate removal, partial import, new formats, or persistence changes.
- Do not claim that the old preview can create duplicates unless that behavior is reproduced; prevent the stale retry regardless.

## Acceptance checks

- [x] Cancel after invalid validation restores the visible file picker without route navigation and moves focus to a useful next action.
- [x] A corrected valid file can be validated and committed during the same Settings visit.
- [x] Successful commit removes or disables the old commit action and moves focus to the verified result.
- [x] A new-file action clears the previous file, preview, confirmation, and transient errors before showing the picker.
- [x] Repeated interaction around completion cannot commit the same preview twice or change the verified record count.
- [x] Existing items, Trash, folders, duplicate handling, and lock settings remain intact through Cancel and successful import.
- [x] Focused public Playwright workflows and relevant import-state Vitest tests pass.

## Dependencies and evidence

No ticket dependencies or backend changes. The audit confirmed that Cancel hides Import until Settings is remounted and that the old commit action remains active after success. Repeat-import duplication is a risk, not a confirmed result.

## Comments

Chromium and Vitest verified Cancel, same-visit correction, single batch completion, clean restart, preserved vault data, and focus after completion with the mobile drawer open.
