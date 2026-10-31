<script lang="ts">
import SignIn from "./_SignIn.svelte";
import { Card, Page, PageHeader } from "$lib/ui";
</script>

<div class="flex min-h-0 grow items-start justify-center overflow-auto p-6 sm:items-center">
	<Page width="narrow">
		<PageHeader
			title="Sign in"
			description="Authenticate with your identity provider to open your vault."
		/>
		<Card>
			<SignIn />
		</Card>
	</Page>
</div>
