import { OidcClient } from "@ofsdk/oidc-client";
import * as v from "valibot";
import { unifiedNode } from "$lib/common/state/nodeSetup";

const intentSchema = v.object({
  issuer: v.string(),
  device: v.string(),
  storage: v.string(),
  clientId: v.string(),
  application: v.string(),
  acceptingPublicKey: v.string(),
  name: v.string(),
  address: v.string(),
});
const viewSchema = v.variant("stage", [
  v.object({ stage: v.literal("setup") }),
  v.object({ stage: v.literal("prepared"), intent: intentSchema, publicKey: v.string() }),
  v.object({
    stage: v.literal("pending"),
    intent: intentSchema,
    publicKey: v.string(),
    deviceId: v.string(),
  }),
  v.object({ stage: v.literal("approved"), publicKey: v.string(), deviceId: v.string() }),
]);
export type JoinIntent = v.InferOutput<typeof intentSchema>;
export type JoinView = v.InferOutput<typeof viewSchema>;

type Action =
  | { action: "prepare"; intent: JoinIntent }
  | { action: "resume" }
  | { action: "request" | "approve"; accessToken: string };

const safeFetch: typeof fetch = (input, init) =>
  fetch(input, { ...init, credentials: "omit", redirect: "error", cache: "no-store" });

function endpoint(base: string): string {
  const url = new URL(base);
  if (location.protocol === "https:" && base === unifiedNode()) return `${url.origin}/setup/join`;
  if (
    location.protocol !== "https:" ||
    url.protocol !== "https:" ||
    !["127.0.0.1", "[::1]"].includes(url.hostname) ||
    !url.port ||
    base !== url.origin ||
    url.origin === location.origin
  ) {
    throw new Error("Use the exact loopback HTTPS join helper URL from the Node operator.");
  }
  return `${url.origin}/setup/join`;
}

export async function joinStatus(base: string): Promise<JoinView> {
  return readView(await safeFetch(endpoint(base), { mode: "cors" }));
}

export async function joinAction(base: string, code: string, action: Action): Promise<JoinView> {
  if (!/^[0-9a-f]{64}$/.test(code)) throw new Error("Enter the Node's exact setup code.");
  return readView(
    await safeFetch(endpoint(base), {
      method: "POST",
      headers: { Authorization: `Setup ${code}`, "Content-Type": "application/json" },
      body: JSON.stringify(action),
      mode: "cors",
    })
  );
}

async function readView(response: Response): Promise<JoinView> {
  if (response.status === 401) throw new Error("Local setup admission was denied.");
  if (!response.ok)
    throw new Error((await response.text()) || "The join helper rejected this request.");
  const parsed = v.safeParse(viewSchema, await response.json());
  if (!parsed.success) throw new Error("The join helper returned invalid setup evidence.");
  return parsed.output;
}

export async function joinClient(intent: JoinIntent, redirectUri: string): Promise<OidcClient> {
  const issuer = new URL(intent.issuer);
  if (
    issuer.protocol !== "https:" ||
    issuer.username ||
    issuer.password ||
    issuer.search ||
    issuer.hash
  ) {
    throw new Error("An exact HTTPS Installation issuer is required.");
  }
  const client = new OidcClient({
    projectOwned: true,
    disableNativeRequests: true,
    storage: "local",
    fetch: safeFetch,
    clientConfig: {
      authority: intent.issuer,
      clientId: intent.clientId,
      redirectUri,
      responseType: "code",
      registration: {
        application: { uri: intent.application },
        clientName: "Join Node",
        tokenEndpointAuthMethod: "none",
        profile: "web_application",
        clientType: "public",
        redirectUris: [redirectUri],
        responseTypes: ["code"],
        scope: "openid profile email",
      },
    },
  });
  const metadata = await client.getOidcConfiguration();
  if (
    metadata.issuer !== intent.issuer ||
    metadata.authorization_endpoint !== `${intent.issuer}/oauth2/auth` ||
    metadata.token_endpoint !== `${intent.issuer}/oauth2/token` ||
    metadata.userinfo_endpoint !== `${intent.issuer}/userinfo`
  ) {
    throw new Error("The Installation OAuth endpoints do not match the retained issuer.");
  }
  return client;
}
