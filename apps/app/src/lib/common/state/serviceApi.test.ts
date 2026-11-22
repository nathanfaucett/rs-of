import { expect, test, vi } from "vitest";
import { discoverServices, validateServiceApiUrl } from "./serviceApi";

function version(name: string): Response {
    return new Response(JSON.stringify({ name }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
    });
}

test("validates the selected owner through its public version endpoint", async () => {
    const fetchApi = vi.fn<typeof fetch>().mockResolvedValue(version("idp-server"));
    await expect(
        validateServiceApiUrl("https://node.test/idp/", "idp", fetchApi),
    ).resolves.toBe(true);
    expect(fetchApi).toHaveBeenCalledWith(
        "https://node.test/idp/version",
        expect.objectContaining({ credentials: "omit", redirect: "error" }),
    );
    await expect(
        validateServiceApiUrl("https://node.test/idp", "device", fetchApi),
    ).resolves.toBe(false);
});

test("discovers all unified prefixes from a base URL", async () => {
    const fetchApi = vi.fn<typeof fetch>().mockImplementation(async (input) => {
        const path = new URL(String(input)).pathname;
        const owner = path.split("/")[1];
        return version(`${owner}-server`);
    });
    await expect(
        discoverServices("https://node.test/idp/", fetchApi),
    ).resolves.toEqual({
        kind: "unified",
        urls: {
            idp: "https://node.test/idp",
            management: "https://node.test/management",
            device: "https://node.test/device",
            storage: "https://node.test/storage",
        },
    });
});

test("identifies a separate service without inferring other hosts", async () => {
    const fetchApi = vi.fn<typeof fetch>().mockImplementation(async (input) => {
        const url = new URL(String(input));
        return url.pathname === "/version"
            ? version("device-server")
            : new Response(null, { status: 404 });
    });
    await expect(
        discoverServices("https://device.test/", fetchApi),
    ).resolves.toEqual({
        kind: "separate",
        owner: "device",
        url: "https://device.test",
    });
});

test("rejects invalid URLs and unrelated version endpoints", async () => {
    const fetchApi = vi.fn<typeof fetch>().mockResolvedValue(version("other-server"));
    await expect(discoverServices("not-a-url", fetchApi)).resolves.toBeNull();
    await expect(
        discoverServices("http://remote.test", fetchApi),
    ).resolves.toBeNull();
    await expect(discoverServices("https://node.test", fetchApi)).resolves.toBeNull();
});
