<script lang="ts" module>
    export interface FieldIssue {
        message: string;
    }

    export interface FieldControl {
        id: string;
        "aria-describedby"?: string;
        "aria-invalid"?: boolean;
        required?: boolean;
    }

    export interface FieldProps {
        label: string;
        description?: string;
        issues?: readonly FieldIssue[];
        required?: boolean;
        children: import("svelte").Snippet<[FieldControl]>;
    }
</script>

<script lang="ts">
    let {
        label,
        description,
        issues = [],
        required = false,
        children,
    }: FieldProps = $props();

    const id = $props.id();
    const descriptionId = $derived(
        description ? `${id}-description` : undefined,
    );
    const issuesId = $derived(issues.length > 0 ? `${id}-issues` : undefined);
    const describedby = $derived(
        [descriptionId, issuesId].filter(Boolean).join(" ") || undefined,
    );
    const invalid = $derived(issues.length > 0);
    const control = $derived<FieldControl>({
        id,
        "aria-describedby": describedby,
        "aria-invalid": invalid ? true : undefined,
        required: required || undefined,
    });
</script>

<div class="field">
    <label class="field-label" for={id}>
        <span class="field-label-text">
            <span>{label}</span>
            {#if required}<span class="field-required" aria-hidden="true"
                    >*</span
                >{/if}
        </span>
        {@render children(control)}
    </label>
    {#if description}
        <p class="field-description" id={descriptionId}>{description}</p>
    {/if}
    {#if issues.length > 0}
        <ul class="field-issues" id={issuesId}>
            {#each issues as issue, index (index)}
                <li>{issue.message}</li>
            {/each}
        </ul>
    {/if}
</div>
