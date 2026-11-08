// Generated type declarations for Alert.svelte
export type AlertTone = "info" | "success" | "warning" | "danger";

export interface AlertProps {
    tone?: AlertTone;
    title?: string;
    children: import("svelte").Snippet;
}

export default Alert;
