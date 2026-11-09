# Map: app-admin-gaps

## Notes

- Spec synthesized from conversation analyzing `of/apps/app` (admin UI) against stated goal: "management of devices, users, clients, and applications"
- `of/apps/password-manager` is a separate app (local password vault), not part of this effort
- Current auth: OIDC against local `of` IdP; no Auth0 integration
- Public-client dynamic registration works; confidential client secret display missing

## Decisions-so-far

1. **User write ops** and **device enrollment** are critical gaps for stated goal
2. **Admin dashboard** is low priority (nice to have)
3. **Confidential client secret display** is medium priority (nice to have)
4. **Non-goals confirmed**: session management, audit logs, Auth0 features, organizations, security policies, branding, backup/monitoring
5. Backend API availability for tickets 01–02 must be verified before implementation

## Fog (Resolved)

- ✅ IdP admin API: `updateUser`, `deleteUser`, `resetPassword` exist; `createUser`, MFA, block/unblock MISSING
- ✅ Device API: `enrollDevice` exists; explicit approve endpoint MISSING
- ✅ `idpApi.getRegister()` returns `clientSecret` — CONFIRMED
- ✅ `idpApi.register()` returns `clientSecret` — CONFIRMED
- ⚠️ Client secret rotation: no dedicated endpoint; `putRegister` may work but untested

## Remaining Fog

- Device approval flow: does `enrollDevice` auto-approve in some cases? Is `IndependentJoinApi.independentJoinApprove()` the correct mechanism?
- `putRegister` for secret rotation: untested; may require current secret for auth
- MFA implementation complexity (TOTP secret generation, QR, WebAuthn ceremony)
