import { render } from "svelte/server";
import { beforeEach, expect, it, vi } from "vitest";
import Page from "./+page.svelte";

const state = vi.hoisted(() => ({
    url: new URL("https://idp.test/authorize"),
}));
vi.mock("$app/state", () => ({ page: state }));
vi.mock("$app/navigation", () => ({ goto: vi.fn(), replaceState: vi.fn() }));
vi.mock("$lib/common/state/idpClient.svelte", () => ({ idpApi: {} }));

beforeEach(() => {
    state.url = new URL(
        "https://idp.test/authorize?response_type=code&response_mode=query&redirect_uri=https%3A%2F%2Fapp.test%2Fcallback&scope=openid",
    );
});

function html() {
    return render(Page, {
        props: {
            data: {
                userInfo: { name: "User", sub: "user", updatedAt: new Date(0) },
            },
        },
    }).body;
}

it("renders an actionable error for an initial request without onboarding metadata", () => {
    expect(html()).toContain(
        "New clients require application_id and registration metadata",
    );
});

it("validates application_id as the registration application URI", () => {
    state.url.searchParams.set("application_id", "https://other.test");
    state.url.searchParams.set(
        "registration",
        JSON.stringify({
            application: { uri: "https://app.test" },
            clientName: "Untrusted client",
            profile: "native_application",
            tokenEndpointAuthMethod: "none",
        }),
    );
    expect(html()).toContain(
        "application_id must match registration.application.uri",
    );
});

it("ignores malformed registration metadata for an existing client and waits for the server", () => {
    state.url.searchParams.set("client_id", "existing-client");
    state.url.searchParams.set(
        "registration",
        "malformed-untrusted-registration",
    );
    const result = html();
    expect(result).toContain("Loading authorization");
    expect(result).not.toContain("Invalid registration metadata");
    expect(result).not.toContain("malformed-untrusted-registration");
});

it("does not offer onboarding for a client ID literally named unknown", () => {
    state.url.searchParams.set("client_id", "unknown");
    expect(html()).toContain("Loading authorization");
    expect(html()).not.toContain(
        "These details were supplied by the application",
    );
});
