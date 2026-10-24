# 06: Separate maintenance from normal onboarding

**What to build:** Users retry ordinary failures without navigating destructive recovery controls. Operators use a separate maintenance path for confirmed local reset, resumable cleanup, advanced exact-reference recovery, and honest remote-revocation status.

**Blocked by:** 01 — Unify onboarding observation and recovery routing.

**Status:** ready-for-agent

- [ ] Normal onboarding and runtime errors present supported prerequisite/retry actions rather than reset as a default fix. Genuine repair-required states can reach an explicit maintenance path without hiding recovery.
- [ ] Destructive reset explains local consequences and requires confirmation. Exact-reference recovery is an advanced, distinct action and never silently replaces missing identity or overwrites conflicting evidence.
- [ ] Reflect reset progress, failure, and exact retry coherently. Stop and join the host before deletion and preserve retained locks and original cleanup references.
- [ ] Protected-provider deletion failure preserves remaining owner data and evidence. Durable data-only cleanup does not repeat successful secret deletion or recreate owners.
- [ ] Distinguish local cleanup completion from pending remote revocation; public targets survive local reset and acknowledgement requires the existing authorized operations on the required surviving peer owners.
- [ ] Keep revocation issuer checks and per-audience authorization intact; do not send a User bearer to a mismatched Installation or report local reset as remote erasure.
- [ ] Reuse proven reset journals and recovery internals. Move shared reference knowledge behind its owning interface only where it eliminates duplication; do not create a general recovery framework or automatic CA-removal feature.
- [ ] Test maintenance navigation and confirmation through existing rendered/native-adapter seams, preserving existing provider refusal, exact-reference conflict, interrupted data cleanup, and partial peer-revocation cases.
- [ ] Delete superseded maintenance presentation and duplicate coordination, update current operations guidance, and record focused validation outcomes without touching real profiles or trust stores.
