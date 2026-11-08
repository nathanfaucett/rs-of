// Generated type declarations for Field.svelte
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
    id?: string;
    label: string;
    description?: string;
    issues?: readonly FieldIssue[];
    required?: boolean;
    children: import("svelte").Snippet<[FieldControl]>;
}

export default Field;
