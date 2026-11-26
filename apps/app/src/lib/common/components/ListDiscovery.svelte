<script lang="ts">
    import type { ListOrder } from "$lib/common/state/listDiscovery";
    import { Field } from "$lib/ui";

    let {
        listScope,
        fields,
        search = $bindable(),
        order = $bindable(),
    }: {
        listScope: string;
        fields: string;
        search: string;
        order: ListOrder;
    } = $props();
</script>

<div class="grid gap-3 sm:grid-cols-2">
    <Field label="Search {listScope}" description={fields}>
        {#snippet children(control)}
            <input type="search" bind:value={search} {...control} />
        {/snippet}
    </Field>
    <Field label="Sort {listScope}">
        {#snippet children(control)}
            <select bind:value={order} {...control}>
                <option value="default">Default order</option>
                <option value="name-asc">Name (A to Z)</option>
                <option value="name-desc">Name (Z to A)</option>
            </select>
        {/snippet}
    </Field>
</div>
