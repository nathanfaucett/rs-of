<script lang="ts">
  import { invoke, isTauri } from "@tauri-apps/api/core";
  import { onMount, onDestroy } from "svelte";
  import { resetDevice, usesRemoteApis } from "../state/setupClient.svelte";
  let busy = $state(false);
  let message = $state("");
  let confirmation = $state("");
  let targets = $state.raw<{ member_id: string; issuer: string; status: string }[]>([]);
  let member = $state("");
  let peer = $state("");
  let bearer = $state("");
  function requireBundled() { if (!isTauri() || usesRemoteApis()) throw new Error("Selected remote APIs cannot use local host recovery"); }
  async function loadTargets() { requireBundled(); targets = await invoke("pending_reset_revocations"); member ||= targets.find(target => target.status === "pending")?.member_id ?? ""; }
  onMount(() => { void loadTargets().catch((cause) => { if (!message) message = `Revocation targets unavailable: ${String(cause)}. Preserve retained evidence; recovery is an explicit advanced action.`; }); });
  async function recover() {
    busy = true;
    message = "Recovering exact retained references…";
    try { requireBundled(); await invoke("recover_reset_references"); message = "Exact reset references recovered, not domain trust. Retry reset explicitly."; }
    catch (cause) { message = String(cause); }
    finally { busy = false; }
  }
  async function reset() {
    if (confirmation !== "RESET") return;
    busy = true;
    message = "Stopping local services and cleaning up this Node…";
    try { requireBundled(); await resetDevice(); message = "Local cleanup complete. Remote revocation is separate; pending targets still require authorized acknowledgement."; await loadTargets().catch((cause) => { message += ` Target refresh failed: ${String(cause)}. Retained revocation status is not confirmed.`; }); } catch (cause) { message = `${String(cause)}. Cleanup is not confirmed complete. Preserve original evidence and retry this same reset explicitly.`; }
    finally { busy = false; confirmation = ""; }
  }
  async function revoke() {
    const token = bearer;
    bearer = "";
    if (!token || !member || !peer) { message = "Choose a retained member, trusted surviving peer and authorized User session."; return; }
    if (!confirm(`Revoke the retained signer and Device on ${peer}?`)) return;
    busy = true;
    try { requireBundled(); await invoke("revoke_reset_on_peer", { peerApi: peer, bearer: token, memberId: member }); await loadTargets(); message = "Surviving peer acknowledged both signer and Device revocation; local reset remains explicit."; }
    catch (cause) { message = String(cause); }
    finally { busy = false; }
  }
  onDestroy(() => { bearer = ""; confirmation = ""; });
</script>
<section aria-labelledby="native-recovery-heading" class="flex flex-col gap-3">
  <h2 id="native-recovery-heading">Local Node maintenance</h2>
  <p>Reset deletes this Node's local data, identity and protected credentials. It does not erase peers or remove OS certificate trust. Failed cleanup retains original evidence for exact retry; do not delete locks or manifests manually.</p>
  <details>
    <summary>Advanced exact-reference recovery</summary>
    <p>Restore only missing exact backed-up reset references. This cannot repair identity, keys or trust, replace missing evidence, or overwrite conflicting evidence. Recovery does not run reset.</p>
    <button onclick={recover} disabled={busy}>Recover reset references</button>
  </details>
  <h3>Remote revocation</h3>
  <p>Public revocation targets survive local reset. Local cleanup is not remote erasure. Only authorized acknowledgement from both surviving peer owners completes revocation. Supply a User bearer authorized for the retained Installation; the peer issuer is checked before it is sent.</p>
  {#if targets.length}
    <label>Retained reset member<select bind:value={member} disabled={busy}>{#each targets as target (target.member_id)}<option value={target.member_id}>{target.member_id} — {target.issuer} — {target.status}</option>{/each}</select></label>
    <label>Surviving peer IdP HTTPS base<input bind:value={peer} autocomplete="off" disabled={busy} /></label>
    <label>Authorized User bearer<input type="password" bind:value={bearer} autocomplete="off" disabled={busy} /></label>
    <button onclick={revoke} disabled={busy || !member || !peer}>Revoke on surviving peer</button>
  {:else}<p>No retained remote revocation targets loaded. This is not proof of remote erasure.</p>{/if}
  <label>Type RESET to confirm<input bind:value={confirmation} autocomplete="off" disabled={busy} /></label>
  <button onclick={reset} disabled={busy || confirmation !== "RESET"}>Reset bundled Node explicitly</button>
  {#if message}<p role="status">{message}</p>{/if}
</section>
