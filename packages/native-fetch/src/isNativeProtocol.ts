export const PROTOCOLS = new Set(["http:", "https:", "about:", "mailto:", "ws:", "wss:"]);

export function isNativeProtocol(url: URL): boolean {
  return !PROTOCOLS.has(url.protocol.toLowerCase());
}
