export async function copyValue(value: string): Promise<boolean> {
  try {
    if (!navigator.clipboard?.writeText) return false;
    await navigator.clipboard.writeText(value);
    return true;
  } catch {
    return false;
  }
}

export function clipboardFeedback(copied: boolean): string {
  return copied
    ? "Copied. Clipboard is not cleared automatically."
    : "Copy failed. Clipboard access is unavailable or denied.";
}
