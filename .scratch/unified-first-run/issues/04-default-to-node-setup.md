# 04: Make Node setup the default for `just unified`

**What to build:** Switch the standard `just unified` and `pnpm apps:dev` workflow to Node setup. Keep seeded startup only as an explicit test fixture. Define how the default handles existing seeded data before switching it. Update the local setup instructions to match.

**Blocked by:** 01 (Create an Installation through Portless), 02 (Join an existing Installation as a Device), 03 (Sign in to Password Manager with the selected Installation).

**Status:** ready-for-agent

- [x] With a fresh data root and no environment overrides, the default Portless URL shows **Start a new Installation** and **Join an existing Installation**, not sign-in.
- [x] After either choice completes, restart retains setup and does not show the first-run choices again. Incomplete setup remains resumable.
- [x] The default startup does not silently adopt, overwrite, or reset existing seeded data. Tests cover the chosen compatibility behavior.
- [x] Real-backend browser tests cover both default setup paths. Tests that need seeded accounts select the fixture explicitly.

## Comments

The default rejects existing seeded roots without changing them. Create and Join share one HTTPS Node and a protected, retained choice. Join does not start local owners. Password Manager uses the real Node descriptor; a joined Installation must allow that app's origin on its own APIs.
