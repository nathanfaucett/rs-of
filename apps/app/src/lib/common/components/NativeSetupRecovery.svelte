<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount, onDestroy } from "svelte";
  import { goto } from "$app/navigation";
  import { resolve } from "$app/paths";
  import { resetDevice, usesRemoteApis } from "../state/setupClient.svelte";
  import { getOidcClient } from "../state/oidc.svelte";
  let { stage, nativeError }: { stage: string; nativeError?: string } = $props();
  let busy = $state(false);
  let message = $state("");
  let confirmation = $state("");
  let targets = $state.raw<{ member_id: string; issuer: string; status: string }[]>([]);
  let member = $state("");
  let peer = $state("");
  let bearer = $state("");
  function requireBundled() { if (usesRemoteApis()) throw new Error("Selected remote APIs cannot use local host recovery"); }
  async function loadTargets() { requireBundled(); targets = await invoke("pending_reset_revocations"); member ||= targets.find(target => target.status === "pending")?.member_id ?? ""; }
  onMount(() => { void loadTargets().catch(() => { message = "Local reset references unavailable; use explicit recovery."; }); });
  async function retry() {
    busy = true;
    try { requireBundled(); await invoke("retry_local_runtime"); await goto(resolve("/setup")); }
    catch (cause) { message = String(cause); }
    finally { busy = false; }
  }
  async function recover() {
    busy = true;
    try { requireBundled(); await invoke("recover_reset_references"); message = "Exact reset references recovered, not domain trust. Retry reset explicitly."; }
    catch (cause) { message = String(cause); }
    finally { busy = false; }
  }
  async function reset() {
    if (confirmation !== "RESET") return;
    busy = true;
    try { await resetDevice(); } catch (cause) { message = String(cause); }
    finally { busy = false; confirmation = ""; }
  }
  async function revoke() {
    const token = bearer || getOidcClient().getStoredTokenResponse()?.access_token;
    bearer = "";
    if (!token || !member || !peer) { message = "Choose a retained member, trusted surviving peer and authorized User session."; return; }
    if (!confirm(`Revoke the retained signer and Management Device on ${peer}?`)) return;
    busy = true;
    try { requireBundled(); await invoke("revoke_reset_on_peer", { peerApi: peer, bearer: token, memberId: member }); await loadTargets(); message = "Surviving peer acknowledged both signer and Device revocation; local reset remains explicit."; }
    catch (cause) { message = String(cause); }
    finally { busy = false; }
  }
  onDestroy(() => { bearer = ""; confirmation = ""; });
</script>
<section aria-labelledby="native-recovery-heading" class="flex flex-col gap-3">
  <h2 id="native-recovery-heading">Bundled host observation and recovery</h2>
  {#if nativeError}<p role="alert">{nativeError}</p>{/if}
  {#if stage === "ready"}
    <p>Local completion does not sign you in or grant permissions.</p><a href={resolve("/signin")}>Sign in</a>
  {:else if stage === "device"}
    <p>Shared HTTPS Installation setup is complete. Finish the local Device preference marker, not authentication enablement.</p><a href={resolve("/setup/device")}>Continue Device setup</a>
  {/if}
  <p>Create, Join, recovery and cancellation use the shared HTTPS worker above. Native capabilities only manage trust, the local host and explicit reset.</p>
  <button onclick={retry} disabled={busy}>Retry local services</button>
  <button onclick={recover} disabled={busy}>Recover reset references</button>
  {#if targets.length}
    <label>Retained reset member<select bind:value={member} disabled={busy}>{#each targets as target (target.member_id)}<option value={target.member_id}>{target.member_id} — {target.issuer} — {target.status}</option>{/each}</select></label>
    <label>Surviving peer IdP HTTPS base<input bind:value={peer} autocomplete="off" disabled={busy} /></label>
    <label>Authorized User bearer<input type="password" bind:value={bearer} autocomplete="off" disabled={busy} /></label>
    <button onclick={revoke} disabled={busy || !member || !peer}>Revoke on surviving peer</button>
  {/if}
  <label>Type RESET to confirm<input bind:value={confirmation} autocomplete="off" disabled={busy} /></label>
  <button onclick={reset} disabled={busy || confirmation !== "RESET"}>Reset bundled Node explicitly</button>
  {#if message}<p role="status">{message}</p>{/if}
</section>
