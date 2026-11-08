<script lang="ts">
import { onDestroy } from "svelte";

let { value, disabled = false, autocomplete = "current-password", ariaLabel = "Toggle password visibility", copyAriaLabel = "Copy to clipboard" }: {
    value: string;
    disabled?: boolean;
    autocomplete?: string;
    ariaLabel?: string;
    copyAriaLabel?: string;
} = $props();

let revealed = $state(true);
let copying = $state(false);
let copied = $state(false);
let active = true;
onDestroy(() => { active = false; });

function toggle() {
    revealed = !revealed;
}

async function copy() {
    if (copying || !value) return;
    copying = true;
    try {
        if (!navigator.clipboard?.writeText) throw new Error();
        await navigator.clipboard.writeText(value);
        if (active) copied = true;
    } catch {
        // Silently fail - user can manually copy
    } finally {
        if (active) copying = false;
        if (copied) {
            setTimeout(() => { if (active) copied = false; }, 1500);
        }
    }
}</script>

<div class="relative">
    <input
        type={revealed ? "text" : "password"}
        bind:value={value}
        {autocomplete}
        {disabled}
        class="flex-1 pr-20"
        aria-describedby="pw-toggle-desc"
    />
    <div class="absolute inset-y-0 right-0 flex items-center gap-1 pr-1">
        <button
            type="button"
            class="flex items-center justify-center w-9 h-full text-muted-foreground hover:text-foreground transition-colors"
            onclick={copy}
            aria-label={copyAriaLabel}
            disabled={disabled || copying || !value}
        >
            {#if copied}
                <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                    <polyline points="20 6 9 17 4 12" />
                </svg>
            {:else if copying}
                <svg class="w-5 h-5 animate-spin" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                    <circle cx="12" cy="12" r="10" stroke-opacity="0.25" />
                    <path d="M12 2a10 10 0 0 1 10 10" />
                </svg>
            {:else}
                <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                    <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                    <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                </svg>
            {/if}
        </button>
        <button
            type="button"
            class="flex items-center justify-center w-9 h-full text-muted-foreground hover:text-foreground transition-colors"
            onclick={toggle}
            aria-label={ariaLabel}
            aria-pressed={revealed}
            disabled={disabled}
        >
            {#if revealed}
                <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                    <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24" />
                    <line x1="1" y1="1" x2="23" y2="23" />
                </svg>
            {:else}
                <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                    <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
                    <circle cx="12" cy="12" r="3" />
                </svg>
            {/if}
        </button>
    </div>
    <span id="pw-toggle-desc" class="sr-only">{revealed ? "Hide password" : "Show password"}</span>
</div>
