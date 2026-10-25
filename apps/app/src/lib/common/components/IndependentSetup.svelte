<script lang="ts">
  import { onDestroy, untrack } from "svelte";
  import { resolve } from "$app/paths";
  import type { SetupHost } from "../state/setupApi";
  import { independentOwners, type OwnerCodes } from "../state/independentSetupApi";
  import { IndependentSetupSession } from "../state/independentSetup.svelte";

  let { hosts }: { hosts: SetupHost[] } = $props();
  const session = untrack(() => new IndependentSetupSession(hosts));
  let purpose = $state<"create" | "join">(untrack(() => hosts.some(host => host.status.state === "joining") ? "join" : "create"));
  let administrator = $state("");
  let node = $state("");
  let peer = $state("");
  let password = $state("");
  let codes = $state<OwnerCodes>({ idp: "", device: "", storage: "" });
  const canSubmit = $derived(!session.busy && !session.ready && !session.observationRequired && (!!session.intent || session.hosts.every(host => host.status.state === "awaiting_choice")));
  function clearSecrets() { password = ""; codes = { idp: "", device: "", storage: "" }; }
  async function submit(event: SubmitEvent) {
    event.preventDefault();
    const currentCodes = { ...codes };
    const credential = password;
    clearSecrets();
    await session.submit(currentCodes, { purpose, administrator, node: purpose === "create" ? session.intent?.node ?? "My Node" : node, peer }, credential);
  }
  async function recover() {
    const currentCodes = { ...codes };
    clearSecrets();
    await session.recover(currentCodes);
    if (session.intent) {
      purpose = session.intent.purpose;
      administrator = session.intent.administrator;
      node = session.intent.node;
      peer = session.intent.peer ?? "";
    }
  }
  async function cancel() {
    const currentCodes = { ...codes };
    clearSecrets();
    await session.cancel(currentCodes);
  }
  async function observe() { clearSecrets(); await session.observe(); }
  onDestroy(() => { clearSecrets(); session.destroy(); });
</script>

<section aria-labelledby="independent-setup-title" class="flex flex-col gap-3">
  <h2 id="independent-setup-title">Independent owner Create / Join</h2>
  <p>Each owner requires its own code in its own HTTPS Authorization header. Codes and passwords are transient, never saved. The deployment must install OS CA trust on browsers and calling hosts; do not bypass certificate warnings.</p>
  <p>One operation targets the exact three bases below. Accepted work continues after this page closes. Partial success is not completion or rollback. After reload, recover all current owner tuples before retrying or canceling; do not invent a replacement nonce.</p>
  <p>For Join, sign in on the source app and use <a href={resolve("/devices")}>Devices → Equal-Node joining</a> to inspect and approve the exact offer. Target setup codes never authorize source approval.</p>
  <form onsubmit={submit} class="flex flex-col gap-3">
    <label>Installation setup choice
      <select bind:value={purpose} onchange={clearSecrets} disabled={session.busy || !!session.intent}>
        <option value="create">Create an Installation</option>
        <option value="join">Join through a trusted peer</option>
      </select>
    </label>
    {#each independentOwners as owner (owner)}
      {@const current = session.hosts.find(host => host.status.owners[0] === owner)}
      <p><strong>{owner}</strong>: {session.bases[owner]} — {current?.status.state}{session.outcomes[owner] ? ` — ${session.outcomes[owner]}` : ""}</p>
      {#if !current?.status.ready}
        <label>{owner} host setup code
          <input type="password" bind:value={codes[owner]} autocomplete="off" spellcheck="false" pattern="[a-f0-9]{64}" minlength="64" maxlength="64" disabled={session.busy} />
        </label>
      {/if}
    {/each}
    {#if purpose === "create"}
      <label>Administrator name<input bind:value={administrator} autocomplete="username" disabled={session.busy || !!session.intent} required /></label>
    {/if}
    {#if purpose === "join"}<label>Node name<input bind:value={node} autocomplete="off" disabled={session.busy || !!session.intent} required /></label>{/if}
    {#if purpose === "join"}
      <label>Trusted source Device HTTPS base#pinned endpoint<input bind:value={peer} autocomplete="off" spellcheck="false" disabled={session.busy || !!session.intent} placeholder="https://source.example/device#…" required /></label>
      <p>The target authenticates the pinned source and discovers its issuer. Do not supply an administrator, issuer or password for Join.</p>
    {:else if !session.hosts.find(host => host.status.owners[0] === "idp")?.status.ready}
      <label>IdP administrator password<input type="password" bind:value={password} autocomplete="new-password" disabled={session.busy} required /></label>
      <p>Only IdP receives this password. Re-enter the original password for an incomplete IdP Create retry.</p>
    {/if}
    {#if session.intent?.issuer}<p>Retained Installation issuer: {session.intent.issuer}</p>{/if}
    <button type="submit" disabled={!canSubmit}>{session.intent ? "Retry exact" : "Start"} {purpose === "create" ? "Create" : "Join"} across owners</button>
    <button type="button" onclick={recover} disabled={session.busy || session.ready}>Recover exact operation across owners</button>
    <button type="button" onclick={cancel} disabled={session.busy || session.ready || !session.intent || !session.intent.issuer || session.observationRequired}>Cancel exact operation across owners</button>
  </form>
  <button type="button" onclick={observe} disabled={session.busy}>Observe all owners and verify readiness</button>
  {#if session.message}<p role={session.observationRequired ? "alert" : "status"}>{session.message}</p>{/if}
  {#if session.ready}<a class="btn" href={resolve("/signin")}>Continue to sign in</a>{/if}
</section>
