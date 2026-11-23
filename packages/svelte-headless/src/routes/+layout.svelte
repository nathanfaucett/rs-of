<script lang="ts">
    import "svelte-highlight/styles/github.css";
    import "./layout.css";

    const { children } = $props();
</script>

<div class="overflow-x-hidden overflow-y-auto">
    <div class="container mx-auto p-4">
        {@render children()}
    </div>
</div>
