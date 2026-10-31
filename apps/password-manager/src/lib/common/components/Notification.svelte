<script lang="ts" module>
interface Props {
    notification: Notification;
}
</script>

<script lang="ts">
    import type { Notification } from "@aicacia/svelte-headless";
    import { Alert } from "$lib/ui";
    import { notifications } from "$lib/common/state/notifications.svelte";

    let { notification }: Props = $props();

    const tone = $derived(
        notification.type === "success"
            ? "success"
            : notification.type === "error"
              ? "danger"
              : notification.type === "warning"
                ? "warning"
                : "info",
    );

    function onDelete() {
        notifications.remove(notification.id);
    }
</script>

<div class="m-1">
    <Alert {tone}>
        <div class="flex items-center justify-between gap-2">
            <span>{notification.message}</span>
            <button
                class="btn btn-icon btn-ghost btn-sm shrink-0"
                type="button"
                onclick={onDelete}
                aria-label="Dismiss notification"
            >
                <span aria-hidden="true">×</span>
            </button>
        </div>
    </Alert>
</div>
