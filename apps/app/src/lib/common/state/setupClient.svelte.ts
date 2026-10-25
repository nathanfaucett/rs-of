import { invoke, isTauri } from "@tauri-apps/api/core";
import { clearOidcSession } from "./oidc.svelte";
import { getIdpApiUrl, usesRemoteApis } from "./idpClient.svelte";
import { observeOnboarding } from "./onboarding";
import {
  cancelSetupCreate,
  setupBase,
  fetchSetupIntent,
  exactJoinRequest,
  submitSetupJoin,
  cancelSetupJoin,
  fetchSetupStatus,
  requireCurrentSetupUiOrigin,
  submitSetupCreate,
  type SetupSessionIntent,
  type SetupStatus,
  verifySetupCode,
  type SetupHost,
} from "./setupApi";

export { usesRemoteApis } from "./idpClient.svelte";

export async function resetDevice(): Promise<void> {
  if (isTauri() && usesRemoteApis())
    throw new Error(
      "Remote owner reset requires explicit protected host device, not local native reset",
    );
  clearOidcSession();
  await invoke("reset_device");
}

export async function readNativeSetupCode(
  base: string,
): Promise<string | null> {
  if (!isTauri() || usesRemoteApis())
    throw new Error(
      "Native capability retrieval is only for the selected bundled host",
    );
  const origin = await invoke<string>("get_localhost_server_base_url");
  const expected = `${setupBase(origin)}/idp`;
  const selected = getIdpApiUrl()?.replace(/\/$/, "");
  if (
    usesRemoteApis() ||
    base !== expected ||
    (selected && selected !== expected)
  )
    throw new Error("Native capability cannot authorize a remote target");
  const code = await invoke<string | null>("get_native_setup_code");
  const current = getIdpApiUrl()?.replace(/\/$/, "");
  if (usesRemoteApis() || (current && current !== expected))
    throw new Error("Selected host changed during capability retrieval");
  if (code !== null && !/^[a-f0-9]{64}$/.test(code))
    throw new Error("Native setup capability unavailable");
  return code;
}

export function isUnifiedSetupHost(host: SetupHost): boolean {
  return (
    host.status.owners.length === 3 &&
    ["idp", "device", "storage"].every((owner) =>
      host.status.owners.includes(owner as "idp" | "device" | "storage"),
    )
  );
}

export class WebSetupCreate {
  readonly base: string;
  status = $state.raw<SetupStatus>();
  intent = $state.raw<Readonly<SetupSessionIntent> | null>(null);
  busy = $state(false);
  observationRequired = $state(false);
  message = $state("");
  private active = true;

  constructor(host: SetupHost) {
    this.base = host.base;
    this.status = host.status;
  }

  private accept(status: SetupStatus): void {
    if (
      !this.status ||
      status.owners.length !== this.status.owners.length ||
      !status.owners.every((owner) => this.status?.owners.includes(owner))
    )
      throw new Error("Setup owner scope changed");
    this.status = status;
    if (status.ready) this.intent = null;
  }

  async submit(
    code: string,
    administrator: string,
    node: string,
    password: string,
  ): Promise<void> {
    if (!this.active || this.busy) return;
    if (
      !this.status ||
      !isUnifiedSetupHost({ base: this.base, status: this.status }) ||
      this.status.stage !== "installation" ||
      this.status.ready ||
      this.observationRequired ||
      (!this.intent && this.status.state !== "awaiting_choice")
    ) {
      this.message =
        "Create is unavailable. Observe the host; do not invent a replacement intent.";
      return;
    }
    if (
      !/^[a-f0-9]{64}$/.test(code) ||
      ![administrator, node].every(
        (value) =>
          value === value.trim() &&
          new TextEncoder().encode(value).byteLength <= 128 &&
          /^\P{Cc}+$/u.test(value),
      ) ||
      new TextEncoder().encode(password).byteLength > 1024 ||
      !/^\P{Cc}+$/u.test(password)
    ) {
      this.message =
        "Enter the exact host code, names of 1–128 UTF-8 bytes without surrounding whitespace, and a password of 1–1024 bytes. Control characters are not allowed.";
      return;
    }
    if (
      this.intent &&
      (!("administrator" in this.intent) ||
        this.intent.administrator !== administrator ||
        this.intent.node !== node)
    ) {
      this.message =
        "Retry requires the exact administrator and Node name retained in this session.";
      return;
    }
    this.busy = true;
    this.message = "";
    try {
      requireCurrentSetupUiOrigin();
      if (!this.intent) {
        const bytes = crypto.getRandomValues(new Uint8Array(16));
        this.intent = Object.freeze({
          operation_nonce: Array.from(bytes, (byte) =>
            byte.toString(16).padStart(2, "0"),
          ).join(""),
          administrator,
          node,
        });
      }
      if (!("administrator" in this.intent))
        throw new Error("Conflicting setup purpose");
      const status = await submitSetupCreate(this.base, code, {
        ...this.intent,
        password,
      });
      if (!this.active) return;
      this.accept(status);
      this.message = status.ready
        ? "Node readiness verified. Setup does not sign you in or grant resource access."
        : "Create accepted or retained; not complete. Observe host status.";
    } catch {
      if (this.active) {
        this.observationRequired = true;
        this.message =
          "Create outcome is uncertain. The host may still be working. Observe status before an exact retry.";
      }
    } finally {
      this.busy = false;
    }
  }

  async join(code: string, node: string, peer: string): Promise<void> {
    if (!this.active || this.busy) return;
    if (
      !this.status ||
      !isUnifiedSetupHost({ base: this.base, status: this.status }) ||
      this.status.stage !== "installation" ||
      this.status.ready ||
      this.observationRequired ||
      (!this.intent && this.status.state !== "awaiting_choice")
    ) {
      this.message =
        "Join is unavailable. Observe or recover the host; do not invent a replacement intent.";
      return;
    }
    try {
      requireCurrentSetupUiOrigin();
      if (!/^[a-f0-9]{64}$/.test(code)) throw new Error("Invalid code");
      exactJoinRequest({ operation_nonce: "0".repeat(32), node, peer });
    } catch {
      this.message =
        "Enter the exact host code, Node name and canonical pinned peer. Verified HTTPS UI is required.";
      return;
    }
    if (
      this.intent &&
      ("administrator" in this.intent ||
        this.intent.node !== node ||
        this.intent.peer !== peer)
    ) {
      this.message =
        "Retry requires the exact retained Join Node and peer, without changing purpose.";
      return;
    }
    this.busy = true;
    this.message = "";
    try {
      if (!this.intent) {
        const bytes = crypto.getRandomValues(new Uint8Array(16));
        this.intent = Object.freeze({
          operation_nonce: Array.from(bytes, (byte) =>
            byte.toString(16).padStart(2, "0"),
          ).join(""),
          node,
          peer,
        });
      }
      if ("administrator" in this.intent)
        throw new Error("Conflicting setup purpose");
      const status = await submitSetupJoin(this.base, code, this.intent);
      if (!this.active) return;
      this.accept(status);
      this.message =
        "Join accepted; not complete. An authorized User on the source must approve the exact offer. Observe status and recover the issuer before cancellation.";
    } catch {
      if (this.active) {
        this.observationRequired = true;
        this.message =
          "Join outcome is uncertain. Observe status or recover intent before an exact retry; the host may still be working.";
      }
    } finally {
      this.busy = false;
    }
  }

  async recover(code: string): Promise<void> {
    if (!this.active || this.busy) return;
    if (
      !this.status ||
      !isUnifiedSetupHost({ base: this.base, status: this.status }) ||
      this.status.ready
    ) {
      this.message =
        "Retained setup recovery is available only for incomplete unified web setup.";
      return;
    }
    if (!/^[a-f0-9]{64}$/.test(code)) {
      this.message = "Enter the exact host code to recover retained setup.";
      return;
    }
    this.busy = true;
    this.message = "";
    try {
      requireCurrentSetupUiOrigin();
      const recovered = await fetchSetupIntent(this.base, code);
      if (!this.active) return;
      const status = await fetchSetupStatus(this.base);
      if (!this.active) return;
      if (
        !isUnifiedSetupHost({ base: this.base, status }) ||
        status.stage === "unavailable" ||
        (recovered === null && status.state !== "awaiting_choice") ||
        (recovered &&
          this.intent &&
          (recovered.operation_nonce !== this.intent.operation_nonce ||
            recovered.node !== this.intent.node ||
            ("administrator" in recovered
              ? !("administrator" in this.intent) ||
                recovered.administrator !== this.intent.administrator
              : !("peer" in this.intent) ||
                recovered.peer !== this.intent.peer ||
                (this.intent.issuer !== undefined &&
                  recovered.issuer !== this.intent.issuer))))
      )
        throw new Error("Retained setup evidence conflicts with this session");
      this.accept(status);
      if (recovered && !status.ready) {
        this.intent = Object.freeze({ ...recovered });
      }
      this.observationRequired = false;
      this.message = status.ready
        ? "Node readiness verified. Setup does not sign you in or grant resource access."
        : recovered
          ? "administrator" in recovered
            ? "Exact retained Create recovered. Re-enter the host code and original password to retry, or the host code to cancel."
            : "Exact retained Join recovered, including authenticated peer and issuer. Re-enter the host code to retry or cancel."
          : this.intent
            ? "No host-bound intent observed. The original session intent is retained; its request may not yet be admitted."
            : "No host-bound setup intent observed. No nonce was generated; choose Create or Join explicitly to start.";
    } catch {
      if (this.active) {
        this.observationRequired = true;
        this.message =
          "Retained setup recovery failed or conflicts with this session. Existing intent is unchanged. Observe status and verify the host code and protected host state.";
      }
    } finally {
      this.busy = false;
    }
  }

  async observe(observed?: SetupStatus): Promise<void> {
    if (!this.active || this.busy) return;
    this.busy = true;
    try {
      requireCurrentSetupUiOrigin();
      const status = observed ?? (await fetchSetupStatus(this.base));
      if (!this.active) return;
      this.accept(status);
      this.observationRequired = false;
      this.message = status.ready
        ? isUnifiedSetupHost({ base: this.base, status })
          ? "Node readiness verified. Setup does not sign you in or grant resource access."
          : "Owner readiness observed; other independent owners remain separate."
        : "Host status observed. Incomplete setup is not readiness.";
    } catch {
      if (this.active) {
        this.observationRequired = true;
        this.message =
          "Status observation failed. Readiness and the operation outcome remain unknown; verify HTTPS trust and host configuration.";
      }
    } finally {
      this.busy = false;
    }
  }

  async cancel(code: string): Promise<void> {
    if (!this.active || this.busy || !this.intent || this.status?.ready) return;
    if ("peer" in this.intent && this.observationRequired) {
      this.message =
        "Observe status or recover exact Join intent before cancellation.";
      return;
    }
    if (!/^[a-f0-9]{64}$/.test(code)) {
      this.message =
        "Enter the exact host code to cancel the retained setup intent.";
      return;
    }
    this.busy = true;
    try {
      requireCurrentSetupUiOrigin();
      if (
        !this.status ||
        !isUnifiedSetupHost({ base: this.base, status: this.status })
      )
        return;
      if ("administrator" in this.intent) {
        await cancelSetupCreate(this.base, code, this.intent);
      } else {
        if (!this.intent.issuer) {
          this.message =
            "Recover the exact Join intent including issuer before cancellation.";
          return;
        }
        await cancelSetupJoin(this.base, code, {
          ...this.intent,
          issuer: this.intent.issuer,
        });
      }
      if (this.active)
        this.message =
          "Cancellation signaled, not cleanup completion. Observe status; the retained intent is unchanged.";
    } catch {
      if (this.active)
        this.message =
          "Cancellation outcome is uncertain. Observe status; do not assume setup stopped.";
    } finally {
      if (this.active) this.observationRequired = true;
      this.busy = false;
    }
  }

  destroy(): void {
    this.active = false;
    this.intent = null;
    this.message = "";
  }
}

export async function verifyWebSetupCode(
  base: string,
  code: string,
): Promise<void> {
  const { hosts } = await observeOnboarding();
  const host = hosts.find((host) => host.base === base);
  if (!host || host.status.stage !== "installation")
    throw new Error(
      "Setup admission target is unavailable or no longer configured",
    );
  await verifySetupCode(host.base, code, host.status.owners.length === 1);
}
