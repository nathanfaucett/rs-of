import type { SetupHost, SetupOwner } from "./setupApi";
import { requireSetupUiOrigin } from "./setupApi";
import {
  cancelIndependentSetup,
  exactIndependentIntent,
  fetchIndependentPublic,
  independentBases,
  independentOwners,
  observeIndependentHosts,
  recoverIndependentIntent,
  sameIndependentIntent,
  submitIndependentSetup,
  verifyIndependentReadiness,
  type IndependentIntent,
  type OwnerCodes,
} from "./independentSetupApi";

export type IndependentChoice = {
  purpose: "create" | "join";
  administrator?: string;
  node: string;
  peer?: string;
};
export class IndependentSetupSession {
  readonly bases: Pick<IndependentIntent, SetupOwner>;
  hosts = $state.raw<SetupHost[]>([]);
  intent = $state.raw<Readonly<IndependentIntent> | null>(null);
  outcomes = $state.raw<Partial<Record<SetupOwner, string>>>({});
  busy = $state(false);
  ready = $state(false);
  observationRequired = $state(false);
  message = $state("");
  private active = true;
  private boundOwners: SetupOwner[] = [];

  constructor(hosts: SetupHost[]) {
    this.bases = Object.freeze(independentBases(hosts));
    this.hosts = hosts;
  }
  private origin() {
    requireSetupUiOrigin(
      typeof window === "undefined" ? "" : window.location.origin,
    );
  }
  private requireCodes(codes: OwnerCodes, owners = independentOwners) {
    if (owners.some((owner) => !/^[a-f0-9]{64}$/.test(codes[owner] ?? "")))
      throw new Error("Enter each incomplete owner's exact code");
  }
  private match(
    values: IndependentIntent[],
    reference = this.intent,
  ): IndependentIntent | null {
    const exact = values[0];
    if (
      exact &&
      (independentOwners.some((owner) => exact[owner] !== this.bases[owner]) ||
        values.some((value) => !sameIndependentIntent(exact, value)) ||
        (reference && !sameIndependentIntent(reference, exact, true)))
    )
      throw new Error(
        "Owner operations conflict with the exact retained intent or issuer",
      );
    return exact ?? null;
  }
  private async current(reference = this.intent): Promise<SetupHost[]> {
    const hosts = await observeIndependentHosts(this.hosts);
    if (
      independentOwners.some(
        (owner) => independentBases(hosts)[owner] !== this.bases[owner],
      )
    )
      throw new Error("Owner bases changed");
    if (hosts.every((host) => host.status.ready)) {
      const evidence = await verifyIndependentReadiness(hosts);
      this.match(
        evidence.map((value) => value.operation),
        reference,
      );
      if (this.active) {
        this.hosts = hosts;
        this.ready = true;
      }
    } else {
      const ready = await Promise.all(
        hosts
          .filter((host) => host.status.ready)
          .map((host) =>
            fetchIndependentPublic(host.base, host.status.owners[0]),
          ),
      );
      this.match(
        ready.map((value) => value!.operation),
        reference,
      );
      if (this.active) {
        this.hosts = hosts;
        this.ready = false;
      }
    }
    return hosts;
  }
  async submit(
    codes: OwnerCodes,
    choice: IndependentChoice,
    password?: string,
  ): Promise<void> {
    if (!this.active || this.busy || this.ready) return;
    this.busy = true;
    try {
      this.origin();
      if (this.observationRequired)
        throw new Error("Observe or recover all owners before an exact retry");
      const current = await this.current();
      if (!this.active || this.ready) return;
      const unfinished = current
        .filter((host) => !host.status.ready)
        .map((host) => host.status.owners[0]);
      this.requireCodes(codes, unfinished);
      if (
        !this.intent &&
        current.some((host) => host.status.state !== "awaiting_choice")
      )
        throw new Error(
          "Recover the retained operation; never invent a replacement nonce",
        );
      const candidate = exactIndependentIntent({
        purpose: choice.purpose,
        operation_nonce: this.intent?.operation_nonce ?? "0".repeat(32),
        administrator:
          choice.purpose === "create" ? (choice.administrator ?? "") : "",
        node: choice.node,
        ...this.bases,
        issuer:
          choice.purpose === "create"
            ? this.bases.idp
            : (this.intent?.issuer ?? ""),
        peer: choice.purpose === "join" ? (choice.peer ?? "") : null,
      });
      if (this.intent && !sameIndependentIntent(candidate, this.intent))
        throw new Error("Retry requires the exact retained choices");
      if (
        choice.purpose === "create" &&
        unfinished.includes("idp") &&
        (typeof password !== "string" ||
          new TextEncoder().encode(password).length < 8 ||
          new TextEncoder().encode(password).length > 4096)
      )
        throw new Error("Re-enter the original 8–4096 byte IdP password");
      if (!this.intent) {
        const bytes = crypto.getRandomValues(new Uint8Array(16));
        candidate.operation_nonce = Array.from(bytes, (byte) =>
          byte.toString(16).padStart(2, "0"),
        ).join("");
        this.intent = Object.freeze(candidate);
      }
      const intent = this.intent;
      const results = await Promise.allSettled(
        unfinished.map((owner) =>
          submitIndependentSetup(
            this.bases[owner],
            owner,
            codes[owner],
            intent,
            password,
          ),
        ),
      );
      if (!this.active) return;
      this.outcomes = Object.fromEntries(
        unfinished.map((owner, index) => [
          owner,
          results[index].status === "fulfilled"
            ? "accepted; not complete"
            : "uncertain; observe before retry",
        ]),
      );
      this.boundOwners = independentOwners.filter(
        (owner) =>
          this.boundOwners.includes(owner) || unfinished.includes(owner),
      );
      this.observationRequired = results.some(
        (result) => result.status === "rejected",
      );
      this.message = this.observationRequired
        ? "Partial or uncertain admission. Accepted owner work continues. Observe or recover all owners before retrying."
        : "Accepted by each incomplete owner; not complete. Observe all owners. Join also requires exact source approval.";
    } catch {
      if (this.active) {
        this.observationRequired = true;
        this.message =
          "Setup not confirmed. Verify inputs/codes, observe or recover all owners; retained intent is unchanged.";
      }
    } finally {
      this.busy = false;
    }
  }
  async observe(): Promise<void> {
    if (!this.active || this.busy) return;
    this.busy = true;
    try {
      this.origin();
      await this.current();
      if (!this.active) return;
      this.observationRequired = false;
      this.message = this.ready
        ? "All owners are ready for the same exact operation and issuer. Sign in normally; setup grants no resource access."
        : "Owner states observed. Partial readiness is not completion. Recover exact intent after reload or before cancellation.";
    } catch {
      if (this.active) {
        this.ready = false;
        this.observationRequired = true;
        this.message =
          "Owner evidence is unavailable, changed or conflicting. Readiness is not established.";
      }
    } finally {
      this.busy = false;
    }
  }
  async recover(codes: OwnerCodes): Promise<void> {
    if (!this.active || this.busy) return;
    this.busy = true;
    try {
      this.origin();
      const current = await this.current();
      if (!this.active || this.ready) return;
      this.requireCodes(
        codes,
        current
          .filter((host) => !host.status.ready)
          .map((host) => host.status.owners[0]),
      );
      const recovered = await Promise.all(
        current.map(async (host) => {
        const owner = host.status.owners[0];
          const value = host.status.ready
            ? (await fetchIndependentPublic(host.base, owner))!.operation
            : await recoverIndependentIntent(host.base, codes[owner]);
          if (!value && host.status.state !== "awaiting_choice")
            throw new Error("Unbound owner disagrees with current status");
        return { owner, value };
        }),
      );
      const exact = this.match(
        recovered.flatMap((result) => (result.value ? [result.value] : [])),
      );
      await this.current(exact ?? this.intent);
      if (!this.active) return;
      if (exact) this.intent = Object.freeze(exact);
      this.boundOwners = recovered
        .filter((result) => result.value !== null)
        .map((result) => result.owner);
      this.observationRequired = false;
      this.message = exact
        ? "Exact operation and authenticated issuer recovered across current owner scopes. Re-enter each incomplete owner's code for retry/cancel; only IdP Create needs the original password."
        : "No bound operation observed. No nonce generated; choose Create or Join explicitly.";
    } catch {
      if (this.active) {
        this.ready = false;
        this.observationRequired = true;
        this.message =
          "Recovery failed or owner tuples/scopes conflict. Retained intent is unchanged; no mutation was resumed.";
      }
    } finally {
      this.busy = false;
    }
  }
  async cancel(codes: OwnerCodes): Promise<void> {
    if (!this.active || this.busy || !this.intent || this.ready) return;
    this.busy = true;
    try {
      this.origin();
      if (this.observationRequired || !this.intent.issuer)
        throw new Error("Recover exact intent before cancellation");
      const current = await this.current();
      const owners = current
        .filter(
          (host) =>
            !host.status.ready &&
            this.boundOwners.includes(host.status.owners[0]),
        )
        .map((host) => host.status.owners[0]);
      this.requireCodes(codes, owners);
      if (!owners.length) throw new Error("No bound active owners");
      const results = await Promise.allSettled(
        owners.map((owner) =>
          cancelIndependentSetup(
            this.bases[owner],
            owner,
            codes[owner],
            this.intent!,
          ),
        ),
      );
      if (!this.active) return;
      this.outcomes = Object.fromEntries(
        owners.map((owner, index) => [
          owner,
          results[index].status === "fulfilled"
            ? "cancellation signaled; cleanup unconfirmed"
            : "cancellation uncertain; observe",
        ]),
      );
      this.message =
        "Cancellation outcomes recorded per owner, not rollback or cleanup completion. Observe all owners; exact intent remains retained.";
    } catch {
      if (this.active)
        this.message =
          "Cancellation not confirmed. Observe/recover exact current owner intent and re-enter the relevant codes.";
    } finally {
      if (this.active) this.observationRequired = true;
      this.busy = false;
  }
}
  destroy(): void {
    this.active = false;
    this.intent = null;
    this.outcomes = {};
    this.message = "";
  }
}
