import { isTauri } from "@tauri-apps/api/core";
import { env } from "$env/dynamic/public";
import { getIdpApiUrl } from "./idpClient.svelte";

export function unifiedNode(): string | undefined {
  const { PUBLIC_UNIFIED_ORIGIN: base } = env;
  if (base && !isTauri() && getIdpApiUrl()?.replace(/\/$/, "") === `${base}/idp`) return base;
  return undefined;
}

export async function nodeSetupStatus() {
  const node = unifiedNode();
  if (!node) return;
  const response = await fetch(`${node}/setup/node`, {
    cache: "no-store",
    credentials: "omit",
    redirect: "error",
    signal: AbortSignal.timeout(5000),
  }).catch(() => undefined);
  if (!response?.ok) return { route: "/setup", state: "unavailable" } as const;
  const descriptor: { state?: unknown; idp?: unknown; storage?: unknown } | undefined =
    await response.json().catch(() => undefined);
  switch (descriptor?.state) {
    case "creating":
      return { route: "/setup/installation" } as const;
    case "joining":
      return { route: "/setup/join" } as const;
    case "ready":
      if (descriptor.idp === `${node}/idp` && descriptor.storage === `${node}/storage`)
        return { route: "/signin" } as const;
      if (typeof descriptor.idp === "string" && typeof descriptor.storage === "string")
        return { route: "/setup/join" } as const;
  }
  const state = descriptor?.state;
  return {
    route: "/setup",
    state: state === "awaiting_choice" || state === "starting" ? state : "unavailable",
  } as const;
}

export async function nodeSetupRoute() {
  return (await nodeSetupStatus())?.route;
}

export async function chooseNodeSetup(choice: "create" | "join", code: string): Promise<void> {
  const node = unifiedNode();
  if (!node || !/^[0-9a-f]{64}$/.test(code)) throw new Error("Enter the Node's exact setup code.");
  const response = await fetch(`${node}/setup/node/choice`, {
    method: "POST",
    cache: "no-store",
    credentials: "omit",
    redirect: "error",
    headers: { Authorization: `Setup ${code}`, "Content-Type": "application/json" },
    body: JSON.stringify(choice),
  });
  if (response.status === 401) throw new Error("The Node setup code was not accepted.");
  if (response.status === 409) throw new Error("The Node already has a different setup choice.");
  if (response.status !== 204) throw new Error("The Node could not retain the setup choice.");
}
