// Generated type declarations for Card.svelte
export type CardTone = "default" | "subtle" | "danger";

export interface CardProps {
    title?: string;
    description?: string;
    tone?: CardTone;
    actions?: import("svelte").Snippet;
    children: import("svelte").Snippet;
}

export default Card;
