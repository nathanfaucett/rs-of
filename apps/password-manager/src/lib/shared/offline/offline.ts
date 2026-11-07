import { base } from "$app/paths";

export function offlineError(cause: unknown): string {
    if (cause instanceof DOMException && cause.name === "QuotaExceededError") {
        return "Storage is full. Free space outside this app, then retry. Do not delete vault storage. Keep an external backup.";
    }
    if (
        cause instanceof DOMException &&
        (cause.name === "SecurityError" || cause.name === "NotAllowedError")
    ) {
        return "Local storage is denied. Allow site storage and use a normal browser profile, then retry. Do not clear existing vault data.";
    }
    return "Offline setup failed. Connect, allow site storage, free storage space if needed, and retry. Close all app tabs to finish an update. The previous cached version and vault storage are not reset.";
}

export async function prepareOffline(): Promise<string> {
    if (!("serviceWorker" in navigator)) {
        return "Offline startup is unavailable. Use a current browser with service workers and HTTPS. Private browsing can restrict storage.";
    }
    const registration = await navigator.serviceWorker.register(
        `${base}/service-worker.js`,
        { type: "module", scope: `${base}/` },
    );
    const installing = registration.installing;
    if (installing) {
        await new Promise<void>((resolve, reject) => {
            const timeout = setTimeout(() => {
                installing.removeEventListener("statechange", changed);
                reject(new Error("Offline install timed out"));
            }, 30000);
            const changed = () => {
                if (
                    (installing.state === "installed" && registration.active) ||
                    installing.state === "activated" ||
                    installing.state === "redundant"
                ) {
                    clearTimeout(timeout);
                    installing.removeEventListener("statechange", changed);
                    if (installing.state === "redundant")
                        reject(new Error("Offline install failed"));
                    else resolve();
                }
            };
            installing.addEventListener("statechange", changed);
            changed();
        });
    }
    if (registration.waiting)
        return "Offline update ready. Lock your vault and close all app tabs, then reopen to use the complete new version.";
    const active = registration.active;
    if (!active) throw new Error("Offline worker unavailable");
    const complete = await new Promise<boolean>((resolve, reject) => {
        const channel = new MessageChannel();
        const timeout = setTimeout(() => {
            channel.port1.close();
            reject(new Error("Offline check timed out"));
        }, 10000);
        channel.port1.onmessage = (event) => {
            clearTimeout(timeout);
            channel.port1.close();
            resolve(event.data.complete === true);
        };
        active.postMessage("offline-status", [channel.port2]);
    });
    return complete
        ? "Offline ready"
        : "Offline assets are missing. Connect and retry. If this continues, install the latest deployment after closing all app tabs. Do not clear vault storage.";
}
