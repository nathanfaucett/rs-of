# 03: Separate vault identity from its name

**What to build:** Local vault creation, selection, and reopen using stable identities so display-name normalization cannot overwrite or merge vaults.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

**Type:** task

**Work status:** open

- [ ] Use stable vault identity separate from display name through storage and the visible selector. Do not use a normalized display name as the sole identity.
- [ ] Handle case, punctuation, whitespace, and duplicate-name collisions explicitly. Creation must never overwrite an existing vault; selection must identify the intended vault without ambiguity.
- [ ] Preserve access to existing named vaults through a safe transition. Failed transitions leave the source usable; remove obsolete naming paths once replaced.
- [ ] Handle concurrent creation or interrupted registration without silently adopting or replacing another vault. Show actionable local-storage errors.
- [ ] Test user-visible create/select/unlock flows and real-persistence close/reopen for colliding names and isolated records. Include storage failure and existing-vault transition cases.
- [ ] Keep this a local-vault concern, not an IdP User, Installation, Node, or organization model.

## Comments
