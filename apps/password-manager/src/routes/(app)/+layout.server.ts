import type { LayoutServerLoad } from "./$types";
import { unlockVault, listVaults, type VaultHandle } from "$lib/vault/vault";
import { createInactivityLock, DEFAULT_LOCK_SETTINGS, type LockSettings } from "$lib/vault/lock";
import { redirect } from "@sveltejs/kit";

export const load: LayoutServerLoad = async ({ cookies, params, url }) => {
	// Get vault ID from session or cookie
	const vaultId = cookies.get("vault_id");
	const masterPassword = cookies.get("master_password"); // In reality, this would be in a secure session

	if (!vaultId || !masterPassword) {
		redirect(302, "/");
	}

	try {
		const vault = await unlockVault(masterPassword, vaultId);
		const lockSettings = await vault.loadLockSettings();
		const inactivity = createInactivityLock(() => {
			// Lock action - would redirect to /
		});

		return {
			vault,
			lockSettings,
			inactivity,
		};
	} catch {
		// Invalid credentials
		cookies.delete("vault_id", { path: "/" });
		cookies.delete("master_password", { path: "/" });
		redirect(302, "/");
	}
};
