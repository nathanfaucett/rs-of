<script lang="ts" module>
    import * as v from "valibot";
    import { m } from "$lib/paraglide/messages";

    const SignInSchema = () =>
        v.object({
            username: v.pipe(
                v.string(),
                v.nonEmpty(m.errors_message_username_required()),
            ),
            password: v.pipe(
                v.string(),
                v.nonEmpty(m.errors_message_password_required()),
                v.minLength(
                    1,
                    m.errors_message_password_min_length({ characters: 1 }),
                ),
            ),
        });
</script>

<script lang="ts">
    import { createForm } from "@aicacia/svelte-forms";
    import { isTauri } from "@tauri-apps/api/core";
    import { afterSigninRedirect } from "$lib/common/state/afterSigninRedirect.svelte";
    import {
        ensureTauriIdpApiUrl,
        passwordToken,
    } from "$lib/common/state/idpClient.svelte";
    import { notifications } from "$lib/common/state/notifications.svelte";
    import {
        ensureOidcClientId,
        getOidcClient,
    } from "$lib/common/state/oidc.svelte";
    import { Field } from "$lib/ui";

    const form = createForm(SignInSchema(), {
        username: "",
        password: "",
    });

    async function onSubmit(e: SubmitEvent) {
        e.preventDefault();

        const [_input, output, error] = await form.validate();

        if (error) {
            return;
        }
        try {
            if (isTauri()) {
                await ensureTauriIdpApiUrl();
            }

            const token = await passwordToken(
                await ensureOidcClientId(),
                output.username,
                output.password,
            );

            getOidcClient().setToken({
                token_type: token.tokenType,
                iss: token.iss,
                scope: token.scope ?? undefined,
                access_token: token.accessToken,
                refresh_token: token.refreshToken,
                refresh_token_expires_in:
                    token.refreshTokenExpiresIn ?? undefined,
                id_token: token.idToken,
                expires_in: token.expiresIn ?? undefined,
            });

            await afterSigninRedirect.onReturn();
        } catch (e) {
            console.error("Error during sign-in:", e);
            notifications.add(
                e instanceof Error ? e.message : m.errors_message_internal(),
            );
        }
    }
</script>

<form onsubmit={onSubmit} class="flex flex-col gap-4">
    <Field
        label={m.signin_username_label()}
        issues={form.fields.username.issues}
    >
        {#snippet children({
            id,
            "aria-describedby": describedby,
            "aria-invalid": invalid,
            required,
        })}
            <input
                {id}
                aria-describedby={describedby}
                aria-invalid={invalid}
                {required}
                type="text"
                aria-label={m.signin_username_label()}
                autocomplete="username"
                placeholder={m.signin_username_placeholder()}
                bind:value={form.fields.username.value}
            />
        {/snippet}
    </Field>
    <Field
        label={m.signin_password_label()}
        issues={form.fields.password.issues}
    >
        {#snippet children({
            id,
            "aria-describedby": describedby,
            "aria-invalid": invalid,
            required,
        })}
            <input
                {id}
                aria-describedby={describedby}
                aria-invalid={invalid}
                {required}
                aria-label={m.signin_password_label()}
                type="password"
                autocomplete="current-password"
                placeholder={m.signin_password_placeholder()}
                bind:value={form.fields.password.value}
            />
        {/snippet}
    </Field>
    <button class="btn btn-primary btn-md mt-1" type="submit"
        >{m.sign_in()}</button
    >
</form>
