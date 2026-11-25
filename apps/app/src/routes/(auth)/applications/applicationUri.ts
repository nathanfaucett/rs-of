export function isApplicationUri(value: string): boolean {
  if (/[\s\p{Cc}<>"{}|\\^`]|%(?![a-f\d]{2})/iu.test(value)) return false;
  if (/^urn:[a-z\d][a-z\d-]{0,30}[a-z\d]:[^/?#]/i.test(value)) return true;
  if (!/^https?:\/\/[^/?#]+/i.test(value)) return false;
  try {
    const url = new URL(value);
    return Boolean(url.hostname && !url.username && !url.password);
  } catch {
    return false;
  }
}
