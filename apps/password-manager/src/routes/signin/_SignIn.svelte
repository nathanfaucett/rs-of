<script lang="ts">
import { getIdpApiUrl, setIdpApiUrl, getSdk } from "$lib/common/state/sdk";
import { Alert, Field } from "$lib/ui";

let error = $state<string | null>(null);
let remoteUrl = $state(getIdpApiUrl() ?? "");

async function signIn(authority: string): Promise<void> {
    error = null;
    try {
        setIdpApiUrl(authority);
        await getSdk().signin();
    } catch {
        error =
            "Unable to sign in. Check the service configuration and try again.";
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

<form class="flex flex-col gap-4" onsubmit={signInWithRemoteServer}>
	<Field
		id="remote-url"
		label="IdP service base URL"
		description="The identity provider that issues your session."
		required
	>
		{#snippet children(control)}
			<input
				{...control}
				bind:value={remoteUrl}
				type="url"
				placeholder="https://example.com/idp"
			/>
		{/snippet}
	</Field>
	<button class="btn btn-primary btn-md self-start" type="submit">
		Sign in
	</button>
	{#if error}
		<Alert tone="danger">{error}</Alert>
	{/if}
</form>
