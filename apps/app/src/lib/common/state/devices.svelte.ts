import {
  Configuration as IndependentJoinConfiguration,
  DefaultApi as IndependentJoinApi,
} from "@aicacia/management-client/owner";
import {
  Configuration as NodeJoinConfiguration,
  DefaultApi as NodeJoinApi,
} from "@aicacia/management-client/node-join";
import * as v from "valibot";
import type {
  DeviceEnrollment,
  DeviceInfo,
  DeviceState,
} from "@aicacia/management-client";
import { ownerAccessToken, ensureTauriIdpApiUrl } from "./idpClient.svelte";
import {
  managementApi,
  getManagementApiUrl,
  ensureTauriManagementApiUrl,
} from "./managementClient.svelte";
import { setupBase } from "./setupApi";
import {
  exactIndependentIntent,
  type IndependentIntent,
} from "./independentSetupApi";

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
    operation?: IndependentIntent;
    device_id?: string;
  };
}

async function nodeJoinBearer(): Promise<string> {
  const token = await ownerAccessToken("idp-management");
  if (!token) throw new Error("Sign in to manage Node joining");
  return token;
}

const nodeJoinInit: RequestInit = {
  credentials: "omit",
  redirect: "error",
  cache: "no-store",
};
async function sourceNodeJoinClient(): Promise<
  NodeJoinApi | IndependentJoinApi
> {
  await ensureTauriIdpApiUrl();
  await ensureTauriManagementApiUrl();
  const base = setupBase(getManagementApiUrl() ?? "");
  const bearer = await nodeJoinBearer();
  if (getManagementApiUrl()?.replace(/\/$/, "") !== base)
    throw new Error(
      "Source owner target changed; sign in to the intended Node",
    );
  if (!base.endsWith("/management"))
    return new IndependentJoinApi(
      new IndependentJoinConfiguration({
        basePath: base,
        fetchApi: fetch,
        headers: { Authorization: `Bearer ${bearer}` },
      }),
    );
  return new NodeJoinApi(
    new NodeJoinConfiguration({
      basePath: base.slice(0, -11),
      fetchApi: fetch,
      headers: { Authorization: `Bearer ${bearer}` },
    }),
  );
}

const unifiedJoinRequest = v.object({
  installation: v.string(),
  member_id: v.pipe(v.string(), v.uuid()),
  endpoint_id: v.string(),
  approving_peer: v.string(),
  name: v.string(),
  public_signer: v.record(v.string(), v.unknown()),
});
const independentJoinRequest = v.object({
  operation: v.custom<IndependentIntent>((value) => {
    try {
      exactIndependentIntent(value, true);
      return true;
    } catch {
      return false;
    }
  }),
  device_id: v.pipe(v.string(), v.uuid()),
  member_id: v.pipe(v.string(), v.uuid()),
  endpoint_id: v.string(),
  public_signer: v.record(v.string(), v.unknown()),
  source: v.object({
    endpoint: v.string(),
    management: v.string(),
    issuer: v.string(),
  }),
});
const pendingJoinSchema = v.object({
  offer_id: v.pipe(v.string(), v.uuid()),
  authenticated_endpoint: v.string(),
  request: v.union([unifiedJoinRequest, independentJoinRequest]),
});

function exactOfferId(offerId: string): string {
  return v.parse(v.pipe(v.string(), v.uuid()), offerId);
}

export async function nodeJoinAccepting(accepting?: boolean): Promise<boolean> {
  const api = await sourceNodeJoinClient();
  const response =
    api instanceof NodeJoinApi
      ? accepting === undefined
        ? await api.acceptanceRaw(nodeJoinInit)
        : await api.setAcceptanceRaw(
            { acceptance: { accepting } },
            nodeJoinInit,
          )
      : accepting === undefined
        ? await api.acceptingRaw(nodeJoinInit)
        : await api.setAcceptingRaw(
            { acceptanceRequest: { accepting } },
            nodeJoinInit,
          );
  if (response.raw.status === 204 && accepting !== undefined)
    return nodeJoinAccepting();
  if (response.raw.status !== 200)
    throw new Error("Invalid source acceptance evidence");
  return v.parse(v.object({ accepting: v.boolean() }), await response.value())
    .accepting;
}

export async function pendingNodeJoins(): Promise<PendingNodeJoin[]> {
  const response = await (await sourceNodeJoinClient()).pendingRaw(
    nodeJoinInit,
  );
  if (response.raw.status !== 200)
    throw new Error("Invalid source offer evidence");
  const offers = v.parse(v.array(pendingJoinSchema), await response.value());
  if (
    offers.some(
      (offer) => offer.authenticated_endpoint !== offer.request.endpoint_id,
    )
  )
    throw new Error("Source offer endpoint binding mismatch");
  return offers.map((offer) => {
    const request = offer.request;
    if (!("operation" in request)) return { ...offer, request };
    if (
      request.operation.purpose !== "join" ||
      request.device_id === request.member_id ||
      request.source.issuer !== request.operation.issuer ||
      `${request.source.management}#${request.source.endpoint}` !==
        request.operation.peer
    )
      throw new Error("Independent offer source/operation binding mismatch");
    return {
      ...offer,
      request: {
        ...request,
        name: request.operation.node,
        installation: request.operation.issuer,
        approving_peer: request.source.endpoint,
      },
    };
  });
}

export async function approvePendingNodeJoin(
  offerId: string,
): Promise<"accepted" | "approved"> {
  const response = await (await sourceNodeJoinClient()).approveRaw(
    { id: exactOfferId(offerId) },
    nodeJoinInit,
  );
  if (response.raw.status !== 200 && response.raw.status !== 202)
    throw new Error("Invalid source approval response; refresh offers");
  return response.raw.status === 202 ? "accepted" : "approved";
}

export async function rejectPendingNodeJoin(offerId: string): Promise<void> {
  const response = await (await sourceNodeJoinClient()).rejectRaw(
    { id: exactOfferId(offerId) },
    nodeJoinInit,
  );
  if (response.raw.status !== 204)
    throw new Error("Invalid source rejection response; refresh offers");
  return;
}

export function listDevices(): Promise<DeviceInfo[]> {
  return managementApi.listDevices();
}

export function renameDevice(id: string, name: string): Promise<DeviceInfo> {
  return managementApi.updateDevice({ id, updateDeviceRequest: { name } });
}

export function revokeDevice(id: string): Promise<void> {
  return managementApi.revokeDevice({ id });
}
