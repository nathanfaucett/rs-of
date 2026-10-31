import { beforeEach, expect, it, vi } from "vitest";
const host = vi.hoisted(() => ({
    native: true,
    remote: false,
    selected: "https://node.test/idp",
    invoke: vi.fn(),
    clear: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({
    isTauri: () => host.native,
    invoke: host.invoke,
}));
vi.mock("./idpClient.svelte", () => ({
    getIdpApiUrl: () => host.selected,
    usesRemoteApis: () => host.remote,
}));
vi.mock("./oidc.svelte", () => ({ clearOidcSession: host.clear }));
const { readNativeSetupCode, resetDevice } = await import(
    "./setupClient.svelte"
);
beforeEach(() => {
    vi.clearAllMocks();
    host.native = true;
    host.remote = false;
    host.selected = "https://node.test/idp";
    host.invoke.mockImplementation(async (command: string) =>
        command === "get_localhost_server_base_url"
            ? "https://node.test"
            : "a".repeat(64),
    );
});
it.each(["idp", "device", "storage"] as const)(
    "retrieves only the selected bundled %s capability with its explicit owner",
    async (owner) => {
        expect(
            await readNativeSetupCode(`https://node.test/${owner}`, owner),
        ).toBe("a".repeat(64));
        expect(host.invoke.mock.calls).toEqual([
            ["get_localhost_server_base_url"],
            ["get_native_setup_code", { owner }],
            ["get_localhost_server_base_url"],
        ]);
    },
);
it("never retrieves a local capability for a remote target", async () => {
    await expect(
        readNativeSetupCode("https://remote.test/device", "device"),
    ).rejects.toThrow(/remote/);
    expect(host.invoke).not.toHaveBeenCalledWith(
        "get_native_setup_code",
        expect.anything(),
    );
    host.remote = true;
    host.invoke.mockClear();
    await expect(
        readNativeSetupCode("https://node.test/idp", "idp"),
    ).rejects.toThrow();
    expect(host.invoke).not.toHaveBeenCalled();
});
it("rejects a changed selection during protected retrieval", async () => {
    host.invoke.mockImplementation(async (command) => {
        if (command === "get_native_setup_code") {
            host.selected = "https://remote.test/idp";
            return "a".repeat(64);
        }
        return "https://node.test";
    });
    await expect(
        readNativeSetupCode("https://node.test/device", "device"),
    ).rejects.toThrow(/changed/);
});
it("preserves retired capabilities as null", async () => {
    host.invoke.mockImplementation(async (command) =>
        command === "get_native_setup_code" ? null : "https://node.test",
    );
    expect(
        await readNativeSetupCode("https://node.test/storage", "storage"),
    ).toBeNull();
});
it("uses protected stopped-owner reset only for bundled selection", async () => {
    await resetDevice();
    expect(host.invoke).toHaveBeenCalledWith("reset_device");
    expect(host.clear).toHaveBeenCalledOnce();
    host.remote = true;
    host.invoke.mockClear();
    host.clear.mockClear();
    await expect(resetDevice()).rejects.toThrow(/protected/);
    expect(host.invoke).not.toHaveBeenCalled();
    expect(host.clear).not.toHaveBeenCalled();
});
