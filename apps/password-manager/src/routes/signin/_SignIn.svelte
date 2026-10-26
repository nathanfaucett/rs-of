<script lang="ts">
import {
    getIdpApiUrl,
    setIdpApiUrl,
    getSdk,
} from "$lib/common/state/sdk";

let error = $state<string | null>(null);
let remoteUrl = $state(getIdpApiUrl() ?? "");

async function signIn(authority: string): Promise<void> {
    error = null;
    try {
        setIdpApiUrl(authority);
        await getSdk().signin();
    } catch {
        error = "Unable to sign in. Check the service configuration and try again.";
    }
}

async function signInWithRemoteServer(event: Event): Promise<void> {
    event.preventDefault();
    const authority = remoteUrl.trim();
    if (!authority) {
        error = "Enter the remote identity provider URL to continue.";
        return;
    }
    await signIn(authority);
}
</script>

<form class="flex flex-col" onsubmit={signInWithRemoteServer}>
	<label for="remote-url">IdP service base URL</label>
	<input id="remote-url" bind:value={remoteUrl} required type="url" placeholder="https://example.com/idp" />
	<button class="btn primary mt-2" type="submit">
		Sign in
	</button>
	{#if error}
		<p class="mt-2 text-sm text-red-600" role="alert">{error}</p>
	{/if}
</form>
