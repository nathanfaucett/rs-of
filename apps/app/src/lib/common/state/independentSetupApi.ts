import { fetchSetupStatus, setupBase, type SetupHost, type SetupOwner } from "./setupApi.ts";

export const independentOwners: SetupOwner[] = ["idp", "management", "storage"];
export type IndependentIntent = {
  purpose: "create" | "join";
  operation_nonce: string;
  administrator: string;
  node: string;
  idp: string;
  management: string;
  storage: string;
  issuer: string;
  peer: string | null;
};
export type OwnerCodes = Record<SetupOwner, string>;
export type IndependentPublic = { operation: IndependentIntent; owner: SetupOwner; facts: Record<string, unknown> };
const keys = "administrator,idp,issuer,management,node,operation_nonce,peer,purpose,storage";

export function exactIndependentPeer(peer: string): string {
  const parts = peer.split("#");
  if (parts.length !== 2 || setupBase(parts[0]) !== parts[0] || !/^[a-f0-9]{64}$/.test(parts[1]) || /^0+$/.test(parts[1]))
    throw new Error("Use the exact source Management HTTPS base#pinned-endpoint");
  return peer;
}

export function exactIndependentIntent(value: unknown, resolved = false): IndependentIntent {
  if (!value || typeof value !== "object" || Array.isArray(value) || Object.keys(value).sort().join(",") !== keys)
    throw new Error("Invalid exact independent setup intent");
  const intent = value as IndependentIntent;
  if (!/^[a-f0-9]{32}$/.test(intent.operation_nonce) || ![intent.administrator, intent.node].every(v => typeof v === "string" && new TextEncoder().encode(v).length <= 256 && !/\p{Cc}/u.test(v)) || !intent.node.trim() || new TextEncoder().encode(JSON.stringify(intent)).length > 1024)
    throw new Error("Invalid exact independent setup intent");
  const bases = independentOwners.map(owner => intent[owner]);
  if (bases.some(base => typeof base !== "string" || setupBase(base) !== base) || new Set(bases).size !== 3)
    throw new Error("Independent owners require distinct exact HTTPS bases");
  if (intent.purpose === "create") {
    if (!intent.administrator.trim() || intent.issuer !== intent.idp || intent.peer !== null)
      throw new Error("Invalid exact Create intent");
  } else if (intent.purpose === "join") {
    if (intent.administrator !== "" || typeof intent.peer !== "string" || (resolved && !intent.issuer)) throw new Error("Invalid exact Join intent");
    exactIndependentPeer(intent.peer);
    if (intent.issuer && setupBase(intent.issuer) !== intent.issuer) throw new Error("Invalid authenticated issuer");
  } else throw new Error("Invalid setup purpose");
  return Object.fromEntries(keys.split(",").map(key => [key, (value as Record<string, unknown>)[key]])) as IndependentIntent;
}

export function sameIndependentIntent(a: IndependentIntent, b: IndependentIntent, unresolved = false): boolean {
  return keys.split(",").every(key => key === "issuer" && unresolved && (!a.issuer || !b.issuer) || a[key as keyof IndependentIntent] === b[key as keyof IndependentIntent]);
}

export function independentBases(hosts: SetupHost[]): Pick<IndependentIntent, SetupOwner> {
  if (hosts.length !== 3) throw new Error("Three independent owner scopes are required");
  const bases = {} as Pick<IndependentIntent, SetupOwner>;
  for (const owner of independentOwners) {
    const matches = hosts.filter(host => host.status.owners.length === 1 && host.status.owners[0] === owner);
    if (matches.length !== 1 || setupBase(matches[0].base) !== matches[0].base) throw new Error("Independent owner scope changed");
    bases[owner] = matches[0].base;
  }
  if (new Set(Object.values(bases)).size !== 3) throw new Error("Independent bases must be distinct");
  return bases;
}

function codeHeader(code: string) {
  if (!/^[a-f0-9]{64}$/.test(code)) throw new Error("Enter each owner's exact host-generated code");
  return { Authorization: `Setup ${code}` };
}
async function readJson(response: Response): Promise<unknown> {
  try { return await response.json(); } catch { throw new Error("Invalid independent setup evidence"); }
}
async function request(base: string, path: string, init: RequestInit): Promise<Response> {
  try {
    return await fetch(`${setupBase(base)}/setup/${path}`, { ...init, credentials: "omit", redirect: "error", cache: "no-store" });
  } catch { throw new Error("Independent setup outcome is uncertain; observe all owners before retrying"); }
}
export async function submitIndependentSetup(base: string, owner: SetupOwner, code: string, value: IndependentIntent, password?: string): Promise<void> {
  const intent = exactIndependentIntent(value);
  if (base !== intent[owner]) throw new Error("Setup target changed");
  const body: object = intent;
  const includePassword = owner === "idp" && intent.purpose === "create";
  if (includePassword && (typeof password !== "string" || new TextEncoder().encode(password).length < 8 || new TextEncoder().encode(password).length > 4096)) throw new Error("IdP Create password must be 8–4096 UTF-8 bytes");
  const response = await request(base, intent.purpose, { method: "POST", headers: { ...codeHeader(code), "Content-Type": "application/json" }, body: JSON.stringify(includePassword ? { ...body, password } : body) });
  if (response.status !== 202) throw new Error(`Independent setup rejected (${response.status}); observe all owners`);
}
export async function recoverIndependentIntent(base: string, code: string): Promise<IndependentIntent | null> {
  const response = await request(base, "intent", { method: "POST", headers: codeHeader(code) });
  if (response.status === 204) return null;
  if (response.status !== 200) throw new Error(`Independent recovery rejected (${response.status})`);
  return exactIndependentIntent(await readJson(response), true);
}
export async function cancelIndependentSetup(base: string, owner: SetupOwner, code: string, value: IndependentIntent): Promise<void> {
  const intent = exactIndependentIntent(value, true);
  if (base !== intent[owner]) throw new Error("Setup target changed");
  const response = await request(base, "cancel", { method: "POST", headers: { ...codeHeader(code), "Content-Type": "application/json" }, body: JSON.stringify(intent) });
  if (response.status !== 204) throw new Error(`Independent cancellation rejected (${response.status}); observe all owners`);
}
export async function fetchIndependentPublic(base: string, owner: SetupOwner, required = true): Promise<IndependentPublic | null> {
  const response = await request(base, "independent/public", {});
  if (response.status === 409 && !required) return null;
  if (response.status !== 200) throw new Error("Owner public operation is unavailable");
  const raw = await readJson(response);
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) throw new Error("Invalid public owner evidence");
  const value = raw as Record<string, unknown>;
  const operation = exactIndependentIntent(value.operation, true);
  if (value.owner !== owner || operation[owner] !== base || !value.facts || typeof value.facts !== "object" || Array.isArray(value.facts)) throw new Error("Owner public operation scope disagrees");
  return { operation, owner, facts: value.facts as Record<string, unknown> };
}
export async function observeIndependentHosts(hosts: SetupHost[]): Promise<SetupHost[]> {
  const bases = independentBases(hosts);
  const current = await Promise.all(independentOwners.map(async owner => ({ base: bases[owner], status: await fetchSetupStatus(bases[owner]) })));
  independentBases(current);
  for (const host of current) if (host.status.stage === "unavailable") throw new Error("Independent owner is unavailable");
  return current;
}
export async function verifyIndependentReadiness(hosts: SetupHost[]): Promise<IndependentPublic[]> {
  const bases = independentBases(hosts);
  if (!hosts.every(host => host.status.ready)) throw new Error("Every intended owner must be ready");
  const values = await Promise.all(independentOwners.map(owner => fetchIndependentPublic(bases[owner], owner)));
  const evidence = values as IndependentPublic[];
  if (evidence.some(value => !sameIndependentIntent(value.operation, evidence[0].operation))) throw new Error("Ready owners disagree on the exact operation or issuer");
  const operation = evidence[0].operation;
  if (independentOwners.some(owner => operation[owner] !== bases[owner])) throw new Error("Ready owner bases disagree");
  return evidence;
}
export function independentLoginClient(evidence: IndependentPublic) {
  const clientId = evidence.facts.login_client;
  if (evidence.owner !== "idp" || typeof clientId !== "string" || !/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/.test(clientId) || /^0{8}-0{4}-0{4}-0{4}-0{12}$/.test(clientId)) throw new Error("Verified public login Client is unavailable");
  return { client_id: clientId, scopes: ["openid"] };
}
