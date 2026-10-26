import { flushSync, mount, unmount } from "svelte";
import { afterEach, expect, it, vi } from "vitest";
import SignIn from "./_SignIn.svelte";

const auth = vi.hoisted(() => ({
    authority: "https://node.test/idp",
    signin: vi.fn(),
    set: vi.fn(),
}));
vi.mock("$lib/common/state/sdk", () => ({
    getIdpApiUrl: () => auth.authority,
    setIdpApiUrl: auth.set,
    getSdk: () => ({ signin: auth.signin }),
}));
let component: ReturnType<typeof mount>;
afterEach(async () => {
    await unmount(component);
    document.body.innerHTML = "";
    vi.clearAllMocks();
});

it("signs in with the explicitly configured IdP service prefix via form submission", async () => {
    component = mount(SignIn, { target: document.body });
    expect(document.body.textContent).toContain("IdP service base URL");
    expect(document.querySelector<HTMLInputElement>("input")?.value).toBe(
        "https://node.test/idp",
    );
    document
        .querySelector("form")
        ?.dispatchEvent(new Event("submit", { cancelable: true }));
    await vi.waitFor(() => expect(auth.signin).toHaveBeenCalledOnce());
    expect(auth.set).toHaveBeenCalledWith("https://node.test/idp");
});

it("surfaces rejected authority configuration rather than rejecting outside the sign-in error boundary", async () => {
    auth.set.mockImplementationOnce(() => {
        throw new Error("IdP API URL must use HTTP or HTTPS");
    });
    component = mount(SignIn, { target: document.body });
    document
        .querySelector("form")
        ?.dispatchEvent(new Event("submit", { cancelable: true }));
    await vi.waitFor(() => {
        flushSync();
        expect(document.querySelector('[role="alert"]')?.textContent).toContain(
            "Unable to sign in",
        );
    });
    expect(auth.signin).not.toHaveBeenCalled();
});
