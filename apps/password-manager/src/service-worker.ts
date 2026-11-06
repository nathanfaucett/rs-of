/// <reference lib="webworker" />
import { base, build, files, version } from "$service-worker";

const worker = self as unknown as ServiceWorkerGlobalScope;
const prefix = `password-manager-app-${base || "/"}-`;
const cacheName = `${prefix}${version}`;
const shell = `${base}/`;
const assets = [...new Set([shell, ...build, ...files])];

async function checkDeployment(): Promise<void> {
    const response = await fetch(`${base}/_app/version.json`, {
        cache: "no-store",
    });
    if (!response.ok || (await response.json()).version !== version) {
        throw new Error(
            "Application deployment changed during offline installation",
        );
    }
}

worker.addEventListener("install", (event) => {
    event.waitUntil(
        (async () => {
            try {
                await checkDeployment();
                const cache = await caches.open(cacheName);
                await cache.addAll(
                    assets.map((url) => new Request(url, { cache: "reload" })),
                );
                await checkDeployment();
            } catch (error) {
                await caches.delete(cacheName);
                throw error;
            }
        })(),
    );
});

worker.addEventListener("activate", (event) => {
    event.waitUntil(
        (async () => {
            // No skipWaiting: existing clients keep their complete asset version until closed.
            for (const name of await caches.keys()) {
                if (name.startsWith(prefix) && name !== cacheName)
                    await caches.delete(name);
            }
            await worker.clients.claim();
        })(),
    );
});

worker.addEventListener("fetch", (event) => {
    const url = new URL(event.request.url);
    if (event.request.method !== "GET" || url.origin !== worker.location.origin)
        return;
    const path =
        event.request.mode === "navigate" &&
        (url.pathname === shell || url.pathname === `${base}/index.html`)
            ? shell
            : url.pathname;
    if (!assets.includes(path)) return;
    event.respondWith(
        (async () => {
            const cache = await caches.open(cacheName);
            // Never fill a damaged version with files from a newer deployment.
            return (
                (await cache.match(path)) ??
                new Response(
                    "Offline assets are missing. Connect and retry offline setup. Do not clear vault storage.",
                    { status: 503, headers: { "Content-Type": "text/plain" } },
                )
            );
        })(),
    );
});

worker.addEventListener("message", (event) => {
    if (event.data !== "offline-status") return;
    event.waitUntil(
        (async () => {
            const cache = await caches.open(cacheName);
            const complete = (
                await Promise.all(assets.map((url) => cache.match(url)))
            ).every(Boolean);
            event.ports[0]?.postMessage({ complete });
        })(),
    );
});
