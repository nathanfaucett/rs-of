import { invoke, isTauri } from "@tauri-apps/api/core";
import { clearOidcSession } from "./oidc.svelte";
import { getIdpApiUrl, usesRemoteApis } from "./idpClient.svelte";
import { setupBase, type SetupOwner } from "./setupApi";
export { usesRemoteApis } from "./idpClient.svelte";
export async function resetDevice(): Promise<void> {
    if (!isTauri() || usesRemoteApis())
        throw new Error("Reset requires protected local owner management");
    clearOidcSession();
    await invoke("reset_device");
}
export async function readNativeSetupCode(
    base: string,
    owner: SetupOwner,
): Promise<string | null> {
    if (!isTauri() || usesRemoteApis())
        throw new Error(
            "Native capability retrieval requires the selected bundled owner",
        );
    const origin = setupBase(
        await invoke<string>("get_localhost_server_base_url"),
    );
    const expected = `${origin}/${owner}`;
    const selected = getIdpApiUrl()?.replace(/\/$/, "");
    if (base !== expected || (selected && selected !== `${origin}/idp`))
        throw new Error("Native capability cannot authorize a remote owner");
    const code = await invoke<string | null>("get_native_setup_code", {
        owner,
    });
    const currentOrigin = setupBase(
        await invoke<string>("get_localhost_server_base_url"),
    );
    const current = getIdpApiUrl()?.replace(/\/$/, "");
    if (usesRemoteApis() || current !== selected || currentOrigin !== origin)
        throw new Error("Selected host changed during capability retrieval");
    if (code !== null && !/^[a-f0-9]{64}$/.test(code))
        throw new Error("Native owner capability unavailable");
    return code;
}
