# Coding Standards

## Svelte 5 Runes Patterns

### Workflow/Collection Initialization

For workflows or collections that need stable identity across renders (e.g., `createLocalSecretWorkflow`), use **lazy `$state` + `$effect`**:

```ts
// ✅ Correct - stable identity, no capture warnings
let workflow = $state<ReturnType<typeof createLocalSecretWorkflow> | null>(null);

$effect(() => {
    if (!workflow) {
        workflow = createLocalSecretWorkflow({ vault });
    }
});

// Usage with null checks
{#if workflow}
    {#if workflow.editor}
        ...
    {:else}
        <p>Initializing...</p>
    {/if}
{:else}
    <p>No editor available</p>
{/if}
```

**Avoid:**
- `$derived.by(() => createWorkflow({ vault }))` — captures `vault` in initial value, triggers warning
- `$state.raw(createWorkflow({ vault }))` — same capture issue

### Reactive Derived Values

Use `$derived` for synchronous computations, `$derived.by` only when you need a function body:

```ts
// ✅ Simple derivation
const filtered = $derived(items.filter(...));

// ✅ Complex derivation needing statements
const filtered = $derived.by(() => {
    const result = items.filter(...);
    return result.sort(...);
});
```

### Effects

Use `$effect` only for side effects (DOM, subscriptions, timers). Never update state inside effects.

```ts
// ✅ Subscription cleanup
$effect(() => {
    const sub = store.subscribe(...);
    return () => sub.unsubscribe();
});

// ❌ Don't update state here
$effect(() => {
    x = y * 2; // Use $derived instead
});
```

### Event Handlers

Use `onclick`/`onchange` attributes, not `on:click`/`on:change`:

```ts
<button onclick={handleClick}>Click</button>
<input onchange={handleChange} />
```

### Snippets Over Slots

Use `{#snippet}` and `{@render}` instead of `<slot>`:

```ts
{#snippet card(title)}
    <div class="card"><h3>{title}</h3></div>
{/snippet}

{@render card("Hello")}
```

### Component Exports

Export types via `.svelte.d.ts` declaration files, not `<script module>` exports in `index.ts`:

```ts
// src/lib/ui/Alert.svelte.d.ts
export type AlertTone = "info" | "success" | "warning" | "danger";
export interface AlertProps { ... }
export default Alert;
```

## TypeScript

- Run `npx tsc --noEmit` before committing
- Fix all errors; warnings are acceptable for pre-existing code
- Use `.d.ts` files for Svelte component type exports

## Testing

- Unit tests: `vitest` in `src/test/`
- Run `npm run test` before committing
- No E2E tests currently (playwriter for manual verification)

## Build

- `npm run build` must pass without errors
- Warnings are acceptable for pre-existing code only
