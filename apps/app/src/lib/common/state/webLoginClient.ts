export function configuredWebLoginClient(
  env: Record<string, string | undefined>,
  selectedIdp: string,
  desktop: boolean,
): string | undefined {
  const clientId = env.PUBLIC_OIDC_CLIENT_ID?.trim();
  if (desktop || !clientId || !env.PUBLIC_IDP_BASE_URL) return undefined;
  try {
    const issuer = new URL(env.PUBLIC_IDP_BASE_URL);
    const loopback = ["localhost", "127.0.0.1", "[::1]"].includes(
      issuer.hostname,
    );
    if (
      issuer.username ||
      issuer.password ||
      issuer.search ||
      issuer.hash ||
      !(
        issuer.protocol === "https:" ||
        (issuer.protocol === "http:" && loopback)
      )
    ) {
      return undefined;
    }
  } catch {
    return undefined;
  }
  return env.PUBLIC_IDP_BASE_URL.replace(/\/$/, "") ===
    selectedIdp.replace(/\/$/, "")
    ? clientId
    : undefined;
}
