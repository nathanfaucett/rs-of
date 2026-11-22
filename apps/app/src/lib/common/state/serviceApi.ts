export const serviceOwners = ["idp", "management", "device", "storage"] as const;
export type ServiceOwner = (typeof serviceOwners)[number];
export type ServiceApiUrls = Record<ServiceOwner, string>;
export type ServiceDiscovery =
    | { kind: "unified"; urls: ServiceApiUrls }
    | { kind: "separate"; owner: ServiceOwner; url: string };

const serviceNames: Record<ServiceOwner, string> = {
    idp: "idp-server",
    management: "management-server",
    device: "device-server",
    storage: "storage-server",
};

function normalizeBase(base: string): string {
    const url = new URL(base);
    const loopback = ["localhost", "127.0.0.1", "[::1]"].includes(
        url.hostname,
    );
    if (
        url.username ||
        url.password ||
        url.search ||
        url.hash ||
        !(
            url.protocol === "https:" ||
            (url.protocol === "http:" && loopback)
        )
    )
        throw new Error("A valid HTTPS or loopback HTTP service URL is required");
    return url.href.replace(/\/+$/, "");
}

async function readServiceName(
    base: string,
    fetchApi: typeof fetch,
): Promise<string | null> {
    try {
        const response = await fetchApi(`${normalizeBase(base)}/version`, {
            credentials: "omit",
            redirect: "error",
            cache: "no-store",
            headers: { Accept: "application/json" },
        });
        if (!response.ok) return null;
        const version: unknown = await response.json();
        if (
            typeof version !== "object" ||
            version === null ||
            !("name" in version) ||
            typeof version.name !== "string"
        )
            return null;
        return version.name;
    } catch {
        return null;
    }
}

export async function validateServiceApiUrl(
    base: string,
    owner: ServiceOwner,
    fetchApi: typeof fetch = fetch,
): Promise<boolean> {
    if (!base) return false;
    return (await readServiceName(base, fetchApi)) === serviceNames[owner];
}

export async function discoverServices(
    input: string,
    fetchApi: typeof fetch = fetch,
): Promise<ServiceDiscovery | null> {
    let base: string;
    try {
        base = normalizeBase(input);
    } catch {
        return null;
    }
    const root = serviceOwners.reduce(
        (candidate, owner) => candidate.replace(new RegExp(`/${owner}$`), ""),
        base,
    );
    const urls = Object.fromEntries(
        serviceOwners.map((owner) => [owner, `${root}/${owner}`]),
    ) as ServiceApiUrls;
    const unified = await Promise.all(
        serviceOwners.map((owner) =>
            validateServiceApiUrl(urls[owner], owner, fetchApi),
        ),
    );
    if (unified.every(Boolean)) return { kind: "unified", urls };

    const name = await readServiceName(base, fetchApi);
    const owner = serviceOwners.find((owner) => serviceNames[owner] === name);
    return owner ? { kind: "separate", owner, url: base } : null;
}
