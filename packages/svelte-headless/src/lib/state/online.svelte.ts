import { BROWSER } from "esm-env";

let online = $state(typeof navigator === "object" ? navigator.onLine : false);

export function isOnline() {
  return online;
}

function onOnline() {
  online = true;
}

function onOffline() {
  online = false;
}

if (BROWSER) {
  window.addEventListener("online", onOnline);
  window.addEventListener("offline", onOffline);
}
