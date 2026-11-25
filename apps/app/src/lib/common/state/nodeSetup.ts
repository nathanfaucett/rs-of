import { isTauri } from "@tauri-apps/api/core";
import * as v from "valibot";
import { env } from "$env/dynamic/public";
import { getIdpApiUrl } from "./idpClient.svelte";

const loginClientSchema = v.object({
  client_id: v.pipe(v.string(), v.nonEmpty()),
  scopes: v.array(v.string()),
});
const intentSchema = v.object({
  administrator: v.pipe(v.string(), v.nonEmpty()),
  password_required: v.boolean(),
  operation_nonce: v.pipe(v.string(), v.nonEmpty()),
});
export type NodeCreateIntent = v.InferOutput<typeof intentSchema>;
type Choice = "create" | "join";
export type NodeSetupStatus =
  | {
      route: "/setup";
      state: "awaiting_choice" | "starting" | "blocked" | "unavailable";
    }
  | { route: "/setup/installation"; state: "creating" }
  | { route: "/setup/join"; state: "joining" | "ready" }
  | {
      route: "/signin";
      state: "ready";
      idp: string;
      storage: string;
      loginClient: v.InferOutput<typeof loginClientSchema>;
    };

const setupInit: RequestInit = {
  cache: "no-store",
  credentials: "omit",
  redirect: "error",
};
let handoff: { node: string; choice: Choice; code: string } | undefined;

export function unifiedNode(): string | undefined {
  const { PUBLIC_UNIFIED_ORIGIN: base } = env;
  if (base && !isTauri() && getIdpApiUrl()?.replace(/\/$/, "") === `${base}/idp`) return base;
  return undefined;
}

export function nodeSetupCode(choice: Choice): string {
  return handoff?.choice === choice && handoff.node === unifiedNode() ? handoff.code : "";
}

export function clearNodeSetupCode(choice: Choice, code: string): void {
  if (code && nodeSetupCode(choice) === code) handoff = undefined;
}

export async function nodeSetupStatus(): Promise<NodeSetupStatus | undefined> {
  const node = unifiedNode();
  if (!node) return;
  const response = await fetch(`${node}/setup/node`, {
    ...setupInit,
    signal: AbortSignal.timeout(5000),
  }).catch(() => undefined);
  const descriptor: {
    state?: unknown;
    idp?: unknown;
    storage?: unknown;
    login_client?: unknown;
  } = (response?.ok && (await response.json().catch(() => undefined))) || {};
  if (node !== unifiedNode()) return { route: "/setup", state: "unavailable" };
  switch (descriptor.state) {
    case "creating":
      return { route: "/setup/installation", state: "creating" };
    case "joining":
      return { route: "/setup/join", state: "joining" };
    case "ready":
      if (descriptor.idp === `${node}/idp` && descriptor.storage === `${node}/storage`) {
        const login = v.safeParse(loginClientSchema, descriptor.login_client);
        if (login.success) {
          clearNodeSetupCode("create", nodeSetupCode("create"));
          return {
            route: "/signin",
            state: "ready",
            idp: descriptor.idp,
            storage: descriptor.storage,
            loginClient: login.output,
          };
        }
      } else if (typeof descriptor.idp === "string" && typeof descriptor.storage === "string") {
        return { route: "/setup/join", state: "ready" };
      }
      break;
    case "awaiting_choice":
    case "starting":
    case "blocked":
      return { route: "/setup", state: descriptor.state };
  }
  return { route: "/setup", state: "unavailable" };
}

export class NodeSetupError extends Error {
  readonly status: number;

  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

async function postNodeSetup(path: string, code: string, body?: unknown): Promise<Response> {
  const node = unifiedNode();
  if (!node || !/^[0-9a-f]{64}$/.test(code)) throw new Error("Enter the Node's exact setup code.");
  const headers = new Headers({ Authorization: `Setup ${code}` });
  if (body !== undefined) headers.set("Content-Type", "application/json");
  const response = await fetch(`${node}/setup/node/${path}`, {
    ...setupInit,
    method: "POST",
    headers,
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
    signal: AbortSignal.timeout(5000),
  }).catch(() => {
    throw new Error("Cannot reach Node setup. Check the connection and check Node status.");
  });
  if (node !== unifiedNode()) throw new Error("The selected Node changed. Check Node status.");
  return response;
}

function requireStatus(response: Response, expected: number, fallback: string): void {
  if (response.status === expected) return;
  throw new NodeSetupError(
    response.status,
    response.status === 401
      ? "The Node setup code was not accepted."
      : response.status === 403
        ? "The Node does not allow setup from this app origin."
        : response.status === 503
          ? "Node setup is unavailable. Check Node status and try again."
          : fallback
  );
}

export async function chooseNodeSetup(choice: Choice, code: string): Promise<void> {
  const node = unifiedNode();
  const response = await postNodeSetup("choice", code, choice);
  requireStatus(
    response,
    204,
    response.status === 409
      ? "The Node already has a different setup choice."
      : "The Node could not retain the setup choice."
  );
  if (node && node === unifiedNode()) handoff = { node, choice, code };
}

export async function readNodeCreateIntent(code: string): Promise<NodeCreateIntent | null> {
  const response = await postNodeSetup("create/intent", code);
  if (response.status === 204) return null;
  requireStatus(response, 200, "Cannot read retained Create setup. Check Node status.");
  const intent = v.safeParse(intentSchema, await response.json().catch(() => undefined));
  if (!intent.success) throw new Error("The Node returned invalid Create recovery information.");
  return intent.output;
}

export async function createNode(code: string, administrator: string, password?: string) {
  const response = await postNodeSetup("create", code, {
    administrator,
    password,
  });
  requireStatus(
    response,
    202,
    response.status === 400
      ? "The Node did not accept the administrator details. Check the username and password."
      : response.status === 409
        ? "The Node did not accept this Create request. Check retained setup and use the original details to retry."
        : "Create could not be confirmed. Check Node status before retrying."
  );
}

export async function cancelNodeCreate(code: string, operationNonce: string) {
  const response = await postNodeSetup("create/cancel", code, {
    operation_nonce: operationNonce,
  });
  requireStatus(
    response,
    204,
    "Create could not be canceled. Check retained setup before retrying."
  );
}
