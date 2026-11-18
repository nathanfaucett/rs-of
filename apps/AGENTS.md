# Application guidance

Follow the root `AGENTS.md` and the north-star workflow in `docs/goal.md`.

- Use Svelte 5 and SvelteKit conventions. Use the available Svelte documentation/skills when creating or changing `.svelte`, `.svelte.ts`, or `.svelte.js` files.
- Keep the password manager small. It is the platform reference application, not a production password-manager product.
- Do not require sibling source checkouts. Build workspace packages with their local scripts.
- Run the affected package's `check`, `test`, and `lint` scripts. Use real-backend browser tests for identity, Storage, Device, or synchronization changes.
- Keep local encrypted vault behavior separate from OIDC authentication and Device authorization.
- Delete obsolete application documentation and component inventories when the owning source changes.
