export function waitMS(ms: number) {
  return new Promise<void>((resolve) => setTimeout(resolve, ms));
}
