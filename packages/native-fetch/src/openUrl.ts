import { isNativeProtocol } from "./isNativeProtocol.js";

export type RedirectOptions = {
  popup?: boolean;
  windowFeatures?: string;
};

export function openUrl(url: URL, options?: RedirectOptions): Window | null {
  if (typeof window === "undefined") {
    return null;
  }

  const urlString = url.toString();
  const isNative = isNativeProtocol(url);

  if (options?.popup && !isNative) {
    if (typeof window.open === "function") {
      return window.open(urlString, "_blank", options.windowFeatures ?? "");
    }
    return null;
  }

  if (window.location) {
    if (isNative) {
      window.location.href = urlString;
    } else {
      window.location.assign(urlString);
    }
  }
  return null;
}
