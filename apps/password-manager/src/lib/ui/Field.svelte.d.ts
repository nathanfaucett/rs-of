// Generated type declarations for Field.svelte
export interface FieldIssue {
  message: string;
}

export interface FieldControl {
  id: string;
  "aria-describedby"?: string | undefined;
  "aria-invalid"?: boolean | undefined;
  required?: boolean | undefined;
}

export interface FieldProps {
  id?: string;
  label: string;
  description?: string;
  issues?: readonly FieldIssue[];
  required?: boolean;
  children: import("svelte").Snippet<[FieldControl]>;
}

declare const Field: import("svelte").Component<FieldProps>;
export default Field;
