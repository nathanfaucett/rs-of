# 06: Separate maintenance from normal onboarding

**What to build:** Users retry ordinary failures without navigating destructive recovery controls. Operators use a separate maintenance path for confirmed local reset, resumable cleanup, advanced exact-reference recovery, and honest remote-revocation status.

**Blocked by:** 01 — Unify onboarding observation and recovery routing.

Status: resolved

- [x] Normal onboarding and runtime errors present supported prerequisite/retry actions rather than reset as a default fix. Genuine repair-required states can reach an explicit maintenance path without hiding recovery.
- [x] Destructive reset explains local consequences and requires confirmation. Exact-reference recovery is an advanced, distinct action and never silently replaces missing identity or overwrites conflicting evidence.
- [x] Reflect reset progress, failure, and exact retry coherently. Stop and join the host before deletion and preserve retained locks and original cleanup references.
- [x] Protected-provider deletion failure preserves remaining owner data and evidence. Durable data-only cleanup does not repeat successful secret deletion or recreate owners.
- [x] Distinguish local cleanup completion from pending remote revocation; public targets survive local reset and acknowledgement requires the existing authorized operations on the required surviving peer owners.
- [x] Keep revocation issuer checks and per-audience authorization intact; do not send a User bearer to a mismatched Installation or report local reset as remote erasure.
- [x] Reuse proven reset journals and recovery internals. Move shared reference knowledge behind its owning interface only where it eliminates duplication; do not create a general recovery framework or automatic CA-removal feature.
- [x] Test maintenance navigation and confirmation through existing rendered/native-adapter seams, preserving existing provider refusal, exact-reference conflict, interrupted data cleanup, and partial peer-revocation cases.
- [x] Delete superseded maintenance presentation and duplicate coordination, update current operations guidance, and record focused validation outcomes without touching real profiles or trust stores.

## Answer

Implemented a dedicated `/maintenance` route with minimal setup/settings links, removing inline maintenance controls and duplicate runtime retry from onboarding. Ordinary failures keep prerequisite/observation/runtime retry; exact repair remains explicitly reachable. Reset explains local deletion, requires confirmation on every attempt, reports progress/failure/exact retry and distinguishes local cleanup from retained pending peer revocation. Advanced exact-reference recovery remains distinct, refuses conflicts through existing native commands and never automatically resets.

Revocation uses an explicit transient User bearer rather than borrowing an unrelated ambient session. Existing issuer checks, per-audience authorization and both-owner acknowledgement remain intact. Proven reset journals, stop/join, provider-first deletion, original references/locks and durable data-only cleanup were preserved without native runtime edits. Browser/remote selection cannot use the maintenance controls.

Updated operations/native handoff and rendered tests. Final focused results: 21 mounted interactions and 28 setup-page tests pass; app build and Svelte check pass; autofix has no issues. Broad auth remains unqualified (latest full run 179 passed/two unchanged-area failures); native tests, feature matrix and CRAP timed out compiling. See [evidence and limits](../evidence/06-maintenance.md). No real profiles/trust stores touched. Merged integration `setup-simplification` tip `23e9006ca` before completion (already up to date).
