export function configuredWebLoginClient(
  env: Record<string, string | undefined>,
  selectedIdp: string,
  desktop: boolean
): string | undefined {
  const { PUBLIC_OIDC_CLIENT_ID: configuredClientId, PUBLIC_IDP_BASE_URL: idpBaseUrl } = env;
  const clientId = configuredClientId?.trim();
  if (desktop || !clientId || !idpBaseUrl) return undefined;
  try {
    const issuer = new URL(idpBaseUrl);
    const loopback = ["localhost", "127.0.0.1", "[::1]"].includes(issuer.hostname);
    if (
      issuer.username ||
      issuer.password ||
      issuer.search ||
      issuer.hash ||
      !(issuer.protocol === "https:" || (issuer.protocol === "http:" && loopback))
    ) {
      return undefined;
    }
  } catch {
    return undefined;
  }
  return idpBaseUrl.replace(/\/$/, "") === selectedIdp.replace(/\/$/, "") ? clientId : undefined;
}
