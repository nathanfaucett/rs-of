import {
  CreateIntentToJSON,
  CreateRequestToJSON,
  JoinIntentToJSON,
  JoinRequestToJSON,
} from "@aicacia/idp-client/setup";

export type SetupStage = "installation" | "device" | "ready" | "unavailable";
export type SetupOwner = "idp" | "management" | "storage";
export type SetupStatus = {
  stage: "installation" | "unavailable" | "ready";
  state:
    | "awaiting_choice"
    | "creating"
    | "joining"
    | "unfinished"
    | "unavailable"
    | "ready";
  ready: boolean;
  owners: SetupOwner[];
  login_client?: { client_id: string; scopes: string[] };
};
export type SetupCreateIntent = {
  operation_nonce: string;
  administrator: string;
  node: string;
};
export type SetupCreateRequest = SetupCreateIntent & { password: string };
export type SetupJoinRequest = {
  operation_nonce: string;
  node: string;
  peer: string;
};
export type SetupJoinIntent = SetupJoinRequest & { issuer: string };
export type SetupIntent = SetupCreateIntent | SetupJoinIntent;
export type SetupSessionIntent =
  SetupCreateIntent | (SetupJoinRequest & { issuer?: string });
export type SetupHost = { base: string; status: SetupStatus };
export type SetupBases = {
  idp: string;
  management?: string | null;
  storage?: string | null;
};

export function setupBase(base: string): string {
  let url: URL;
  try {
    url = new URL(base);
  } catch {
    throw new Error("A verified HTTPS service base is required for setup");
  }
  if (
    url.protocol !== "https:" ||
    url.username ||
    url.password ||
    url.search ||
    url.hash
  ) {
    throw new Error("A verified HTTPS service base is required for setup");
  }
  return url.href.replace(/\/$/, "");
}

export function requireSetupUiOrigin(origin: string): void {
  let url: URL;
  try {
    url = new URL(origin);
  } catch {
    throw new Error("Setup requires the current HTTPS UI origin");
  }
  if (url.protocol !== "https:" || url.origin !== origin)
    throw new Error("Setup requires the current HTTPS UI origin");
}

const owners: SetupOwner[] = ["idp", "management", "storage"];

export async function fetchSetupStatus(base: string): Promise<SetupStatus> {
  const url = `${setupBase(base)}/setup/status`;
  let response: Response;
  try {
    response = await fetch(url, {
      credentials: "omit",
      redirect: "error",
      cache: "no-store",
    });
  } catch {
    throw new Error(
      "Setup status request failed; verify HTTPS trust and UI origin configuration",
    );
  }
  if (response.status !== 200 && response.status !== 503)
    throw new Error(`Setup status unavailable (${response.status})`);
  return readSetupStatus(response);
}

async function readSetupStatus(response: Response): Promise<SetupStatus> {
  let status: unknown;
  try {
    status = await response.json();
  } catch {
    throw new Error("Setup evidence is invalid");
  }
  if (!status || typeof status !== "object" || Array.isArray(status))
    throw new Error("Setup evidence is invalid");
  const value = status as Record<string, unknown>;
  if (
    value.stage !== "installation" ||
    typeof value.ready !== "boolean" ||
    ![
      "awaiting_choice",
      "creating",
      "joining",
      "unfinished",
      "unavailable",
      "ready",
    ].includes(value.state as string) ||
    value.ready !== (value.state === "ready") ||
    (response.status === 503 && value.state !== "unavailable") ||
    !Array.isArray(value.owners) ||
    (value.owners.length !== 1 && value.owners.length !== 3) ||
    value.owners.some((owner) => !owners.includes(owner)) ||
    new Set(value.owners).size !== value.owners.length
  ) {
    throw new Error("Setup evidence is invalid");
  }
  let login: SetupStatus["login_client"];
  if (value.ready && value.owners.length === 3 && value.owners.includes("idp")) {
    const client = value.login_client as Record<string, unknown> | undefined;
    if (
      response.status !== 200 ||
      !client ||
      typeof client.client_id !== "string" ||
      !/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/.test(
        client.client_id,
      ) ||
      client.client_id === "00000000-0000-0000-0000-000000000000" ||
      !Array.isArray(client.scopes) ||
      !client.scopes.includes("openid") ||
      client.scopes.some(
        (scope) => typeof scope !== "string" || !scope || /\s/.test(scope),
      ) ||
      new Set(client.scopes).size !== client.scopes.length
    )
      throw new Error("Setup evidence is invalid");
    login = {
      client_id: client.client_id,
      scopes: client.scopes as string[],
    };
  } else if (value.login_client != null) {
    throw new Error("Setup evidence is invalid");
  }
  return {
    stage: value.ready
      ? "ready"
      : value.state === "unavailable"
        ? "unavailable"
        : "installation",
    state: value.state as SetupStatus["state"],
    ready: value.ready,
    owners: owners.filter((owner) =>
      (value.owners as unknown[]).includes(owner),
    ),
    ...(login ? { login_client: login } : {}),
  };
}

export async function fetchSetupStage(base: string): Promise<SetupStage> {
  return (await fetchSetupStatus(base)).stage;
}

export async function fetchSetupHosts(bases: SetupBases): Promise<SetupHost[]> {
  const base = setupBase(bases.idp);
  const status = await fetchSetupStatus(base);
  if (status.owners.length === 3) return [{ base, status }];
  if (status.owners[0] !== "idp")
    throw new Error("Primary setup scope must include IdP");
  if (!bases.management || !bases.storage)
    throw new Error(
      "Independent setup requires configured Management and Storage HTTPS service bases",
    );
  const management = setupBase(bases.management);
  const storage = setupBase(bases.storage);
  if (new Set([base, management, storage]).size !== 3)
    throw new Error("Independent setup service bases must be distinct");
  const hosts: SetupHost[] = [{ base, status }];
  for (const [owner, target] of [
    ["management", management],
    ["storage", storage],
  ] as const) {
    const targetStatus = await fetchSetupStatus(target);
    if (targetStatus.owners.length !== 1 || targetStatus.owners[0] !== owner)
      throw new Error(
        `Setup scope does not match the configured ${owner} service`,
      );
    hosts.push({ base: target, status: targetStatus });
  }
  if (hosts.every(host => host.status.ready)) {
    const { verifyIndependentReadiness, independentLoginClient } = await import("./independentSetupApi.ts");
    const evidence = await verifyIndependentReadiness(hosts);
    hosts[0].status.login_client = independentLoginClient(evidence[0]);
  }
  return hosts;
}

function requireSetupCode(code: string): void {
  if (!/^[a-f0-9]{64}$/.test(code))
    throw new Error("Enter the exact host-generated setup code");
}

function requireHumanIntent(nonce: string, names: string[]): void {
  if (
    !/^[a-f0-9]{32}$/.test(nonce) ||
    !names.every(
      (value) =>
        typeof value === "string" &&
        value === value.trim() &&
        new TextEncoder().encode(value).byteLength <= 128 &&
        /^\P{Cc}+$/u.test(value),
    )
  )
    throw new Error("Invalid exact setup intent");
}

function exactCreateIntent(intent: SetupCreateIntent): SetupCreateIntent {
  requireHumanIntent(intent.operation_nonce, [
    intent.administrator,
    intent.node,
  ]);
  return {
    operation_nonce: intent.operation_nonce,
    administrator: intent.administrator,
    node: intent.node,
  };
}

async function setupMutation(
  base: string,
  code: string,
  action: "create" | "join" | "cancel",
  body: object,
): Promise<Response> {
  requireSetupCode(code);
  const url = `${setupBase(base)}/setup/${action}`;
  try {
    return await fetch(url, {
      method: "POST",
      headers: {
        Authorization: `Setup ${code}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify(body),
      credentials: "omit",
      redirect: "error",
      cache: "no-store",
    });
  } catch {
    throw new Error(
      "Setup request failed; the host may still be working. Observe status before an exact retry",
    );
  }
}

export async function submitSetupCreate(
  base: string,
  code: string,
  request: SetupCreateRequest,
): Promise<SetupStatus> {
  const intent = exactCreateIntent(request);
  if (
    typeof request.password !== "string" ||
    new TextEncoder().encode(request.password).byteLength > 1024 ||
    !/^\P{Cc}+$/u.test(request.password)
  )
    throw new Error("Invalid protected administrator password");
  const response = await setupMutation(
    base,
    code,
    "create",
    CreateRequestToJSON({
      operationNonce: intent.operation_nonce,
      administrator: intent.administrator,
      node: intent.node,
      password: request.password,
    }),
  );
  if (response.status !== 202 && response.status !== 200)
    throw new Error(
      `Setup Create rejected (${response.status}); observe status before an exact retry`,
    );
  const status = await readSetupStatus(response);
  if (
    status.owners.length !== 3 ||
    (response.status === 202 &&
      (status.ready || status.state !== "creating")) ||
    (response.status === 200 && !status.ready)
  )
    throw new Error("Setup evidence is invalid");
  return status;
}

export async function cancelSetupCreate(
  base: string,
  code: string,
  intent: SetupCreateIntent,
): Promise<void> {
  const response = await setupMutation(
    base,
    code,
    "cancel",
    CreateIntentToJSON({
      operationNonce: exactCreateIntent(intent).operation_nonce,
      administrator: intent.administrator,
      node: intent.node,
    }),
  );
  if (response.status !== 204)
    throw new Error(
      `Setup cancellation rejected (${response.status}); observe status`,
    );
}

export async function fetchSetupIntent(
  base: string,
  code: string,
): Promise<SetupIntent | null> {
  requireSetupCode(code);
  const url = `${setupBase(base)}/setup/intent`;
  let response: Response;
  try {
    response = await fetch(url, {
      method: "POST",
      headers: { Authorization: `Setup ${code}` },
      credentials: "omit",
      redirect: "error",
      cache: "no-store",
    });
  } catch {
    throw new Error(
      "Setup intent recovery failed; verify HTTPS trust and host configuration",
    );
  }
  if (response.status === 204) return null;
  if (response.status !== 200)
    throw new Error(`Setup intent recovery rejected (${response.status})`);
  try {
    const value: unknown = await response.json();
    if (!value || typeof value !== "object" || Array.isArray(value))
      throw new Error("Invalid retained setup intent");
    const keys = Object.keys(value).sort().join(",");
    if (keys === "administrator,node,operation_nonce")
      return exactCreateIntent(value as SetupCreateIntent);
    if (keys === "issuer,node,operation_nonce,peer")
      return exactJoinIntent(value as SetupJoinIntent);
    throw new Error("Invalid retained setup intent");
  } catch {
    throw new Error("Invalid retained setup intent");
  }
}

export function exactJoinRequest(request: SetupJoinRequest): SetupJoinRequest {
  requireHumanIntent(request.operation_nonce, [request.node]);
  if (!/^[a-f0-9]{64}$/.test(request.peer) || /^0+$/.test(request.peer))
    throw new Error("Invalid canonical pinned peer");
  return {
    operation_nonce: request.operation_nonce,
    node: request.node,
    peer: request.peer,
  };
}

function exactJoinIntent(intent: SetupJoinIntent): SetupJoinIntent {
  const request = exactJoinRequest(intent);
  if (
    typeof intent.issuer !== "string" ||
    intent.issuer !== intent.issuer.trim()
  )
    throw new Error("Invalid retained Join issuer");
  setupBase(intent.issuer);
  return { ...request, issuer: intent.issuer };
}

export async function submitSetupJoin(
  base: string,
  code: string,
  request: SetupJoinRequest,
): Promise<SetupStatus> {
  const intent = exactJoinRequest(request);
  const response = await setupMutation(
    base,
    code,
    "join",
    JoinRequestToJSON({
      operationNonce: intent.operation_nonce,
      node: intent.node,
      peer: intent.peer,
    }),
  );
  if (response.status !== 202)
    throw new Error(
      `Setup Join rejected (${response.status}); observe status before an exact retry`,
    );
  const status = await readSetupStatus(response);
  if (status.owners.length !== 3 || status.ready || status.state !== "joining")
    throw new Error("Setup evidence is invalid");
  return status;
}

export async function cancelSetupJoin(
  base: string,
  code: string,
  intent: SetupJoinIntent,
): Promise<void> {
  const exact = exactJoinIntent(intent);
  const response = await setupMutation(
    base,
    code,
    "cancel",
    JoinIntentToJSON({
      operationNonce: exact.operation_nonce,
      node: exact.node,
      peer: exact.peer,
      issuer: exact.issuer,
    }),
  );
  if (response.status !== 204)
    throw new Error(
      `Setup cancellation rejected (${response.status}); observe status`,
    );
}

export async function verifySetupCode(
  base: string,
  code: string,
): Promise<void> {
  requireSetupCode(code);
  const url = `${setupBase(base)}/setup/admission`;
  let response: Response;
  try {
    response = await fetch(url, {
      method: "POST",
      headers: { Authorization: `Setup ${code}` },
      credentials: "omit",
      redirect: "error",
      cache: "no-store",
    });
  } catch {
    throw new Error(
      "Setup admission request failed; verify HTTPS trust and UI origin configuration",
    );
  }
  if (response.status !== 204)
    throw new Error(`Setup admission rejected (${response.status})`);
}
