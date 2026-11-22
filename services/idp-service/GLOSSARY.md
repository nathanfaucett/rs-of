# Identity Provider glossary

**Identity Provider (IdP)**:
The OAuth 2.0 and OpenID Connect authority for Users, credentials, Applications, OAuth Clients, grants, and token-signing trust. Device records belong to the Device context.

**Issuer key**:
A key used by the IdP authority to sign tokens for its issuer. Verifiers use only its public material. An issuer key does not grant Device or Resource access.

**Canonical Issuer**:
The stable public IdP URL that identifies an Installation in OAuth/OIDC metadata and tokens.

**OAuth Client**:
A web, native, or machine integration for an Application, with its own client identity and authorization configuration. Multiple Clients may access one User/Application namespace when authorized.

**OAuth Consent**:
A User's approval for a Client, redirect URI, and scope set.

**Authorization Code**:
A short-lived OAuth credential bound to its Client, redirect URI, scopes, and optional PKCE and OIDC values.

**Access Token**:
A signed credential that authorizes an API request within its grant constraints.

**ID Token**:
A signed credential that conveys authenticated OIDC identity claims to a Client.

**Refresh Token**:
A signed credential for obtaining replacement tokens under its original grant constraints.

**Key**:
Signing-key metadata, including identity, derivation relationship, state, validity, and any publishable public JWK.
