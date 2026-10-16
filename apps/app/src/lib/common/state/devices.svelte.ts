import { invoke, isTauri } from "@tauri-apps/api/core";
import * as v from "valibot";
import type {
  DeviceEnrollment,
  DeviceInfo,
  DeviceState,
} from "@aicacia/idp-client";
import { idpApi, idpOwnerAccessToken, getIdpApiUrl } from "./idpClient.svelte";
import { setupBase } from "./setupApi";

export type { DeviceEnrollment, DeviceInfo, DeviceState };

export interface PendingNodeJoin {
  offer_id: string;
  authenticated_endpoint: string;
  request: {
    installation: string;
    member_id: string;
    endpoint_id: string;
    approving_peer: string;
    name: string;
    public_signer: Record<string, unknown>;
  };
}

async function nodeJoinBearer(): Promise<string> {
  const token = await idpOwnerAccessToken();
  if (!token) throw new Error("Sign in to manage Node joining");
  return token;
}

async function webNodeJoinRequest(
  path: string,
  method: "GET" | "PUT" | "POST",
  body?: object,
): Promise<Response> {
  const base = setupBase(getIdpApiUrl() ?? "");
  const bearer = await nodeJoinBearer();
  if (getIdpApiUrl()?.replace(/\/$/, "") !== base)
    throw new Error(
      "Source owner target changed; sign in to the intended Node",
    );
  let response: Response;
  try {
    response = await fetch(`${base}/node-joins/${path}`, {
      method,
      headers: {
        Authorization: `Bearer ${bearer}`,
        ...(body ? { "Content-Type": "application/json" } : {}),
      },
      ...(body ? { body: JSON.stringify(body) } : {}),
      credentials: "omit",
      redirect: "error",
      cache: "no-store",
    });
  } catch {
    throw new Error(
      "Source Node request outcome is uncertain; refresh acceptance and offers before retrying",
    );
  }
  if (!response.ok)
    throw new Error(
      `Source Node request denied (${response.status}); refresh acceptance and offers`,
    );
  return response;
}

const pendingJoinSchema = v.object({
  offer_id: v.pipe(v.string(), v.uuid()),
  authenticated_endpoint: v.string(),
  request: v.object({
    installation: v.string(),
    member_id: v.pipe(v.string(), v.uuid()),
    endpoint_id: v.string(),
    approving_peer: v.string(),
    name: v.string(),
    public_signer: v.record(v.string(), v.unknown()),
  }),
});

function exactOfferId(offerId: string): string {
  return v.parse(v.pipe(v.string(), v.uuid()), offerId);
}

export async function nodeJoinAccepting(accepting?: boolean): Promise<boolean> {
  if (!isTauri()) {
    const response = await webNodeJoinRequest(
      "acceptance",
      accepting === undefined ? "GET" : "PUT",
      accepting === undefined ? undefined : { accepting },
    );
    if (response.status !== 200)
      throw new Error("Invalid source acceptance evidence");
    return v.parse(v.object({ accepting: v.boolean() }), await response.json())
      .accepting;
  }
  return invoke("node_join_accepting", {
    bearer: await nodeJoinBearer(),
    accepting,
  });
}

export async function pendingNodeJoins(): Promise<PendingNodeJoin[]> {
  if (!isTauri()) {
    const response = await webNodeJoinRequest("pending", "GET");
    if (response.status !== 200)
      throw new Error("Invalid source offer evidence");
    const offers = v.parse(v.array(pendingJoinSchema), await response.json());
    if (
      offers.some(
        (offer) => offer.authenticated_endpoint !== offer.request.endpoint_id,
      )
    )
      throw new Error("Source offer endpoint binding mismatch");
    return offers;
  }
  return invoke("pending_node_joins", { bearer: await nodeJoinBearer() });
}

export async function approvePendingNodeJoin(offerId: string): Promise<void> {
  if (!isTauri()) {
    const response = await webNodeJoinRequest(
      `${exactOfferId(offerId)}/approve`,
      "POST",
    );
    if (response.status !== 200)
      throw new Error("Invalid source approval response; refresh offers");
    return;
  }
  return invoke("approve_pending_node_join", {
    bearer: await nodeJoinBearer(),
    offerId,
  });
}

export async function rejectPendingNodeJoin(offerId: string): Promise<void> {
  if (!isTauri()) {
    const response = await webNodeJoinRequest(
      `${exactOfferId(offerId)}/reject`,
      "POST",
    );
    if (response.status !== 204)
      throw new Error("Invalid source rejection response; refresh offers");
    return;
  }
  return invoke("reject_pending_node_join", {
    bearer: await nodeJoinBearer(),
    offerId,
  });
}

export function listDevices(): Promise<DeviceInfo[]> {
  return idpApi.listDevices();
}

export function renameDevice(id: string, name: string): Promise<DeviceInfo> {
  return idpApi.updateDevice({ id, updateDeviceRequest: { name } });
}

export function revokeDevice(id: string): Promise<void> {
  return idpApi.revokeDevice({ id });
}
