import { flushSync, mount, unmount } from "svelte";
import { afterEach, expect, it, vi } from "vitest";
import IndependentSetup from "./IndependentSetup.svelte";
import type { SetupHost } from "../state/setupApi";

vi.mock("$app/paths", () => ({ resolve: (path: string) => path }));
const bases = { idp: "https://target.test/idp", management: "https://target.test/management", storage: "https://target.test/storage" };
const owners = ["idp", "management", "storage"] as const;
let component: ReturnType<typeof mount>;
let target: HTMLDivElement;
afterEach(async () => { if (component) await unmount(component); target?.remove(); vi.unstubAllGlobals(); });
function input(label: string) {
  const element = [...target.querySelectorAll("label")].find(node => node.textContent?.includes(label))?.querySelector<HTMLInputElement | HTMLSelectElement>("input,select");
  if (!element) throw new Error("Missing input " + label);
  return element;
}
function enter(label: string, value: string) {
  const element = input(label); element.value = value;
  element.dispatchEvent(new Event(element.tagName === "SELECT" ? "change" : "input", { bubbles: true })); flushSync();
}
function button(label: string) { return [...target.querySelectorAll("button")].find(node => node.textContent?.includes(label))!; }

it.each(["create", "join"] as const)("drives independent %s from UI through transient inputs, exact requests and correlated readiness", async purpose => {
  target = document.createElement("div"); document.body.append(target);
  let operation: Record<string, unknown> | undefined;
  let ready = false;
  const fetchApi = vi.fn(async (url: string, init: RequestInit = {}) => {
    const owner = owners.find(owner => url.startsWith(bases[owner] + "/"))!;
    if (url.endsWith("/status")) return Response.json({ stage: "installation", state: ready ? "ready" : "awaiting_choice", ready, owners: [owner] });
    if (url.endsWith("/public")) return Response.json({ owner, operation, facts: owner === "idp" ? { login_client: "01900000-0000-7000-8000-000000000001" } : {} });
    const { password: _password, ...intent } = JSON.parse(init.body as string);
    operation = { ...intent, issuer: "https://installation.test/idp" };
    if (purpose === "create") operation.issuer = bases.idp;
    return new Response(null, { status: 202 });
  });
  vi.stubGlobal("fetch", fetchApi);
  const hosts: SetupHost[] = owners.map(owner => ({ base: bases[owner], status: { stage: "installation", state: "awaiting_choice", ready: false, owners: [owner] } }));
  component = mount(IndependentSetup, { target, props: { hosts } }); flushSync();
  enter("Installation setup choice", purpose);
  for (let index = 0; index < owners.length; index++) enter(owners[index] + " host setup code", String.fromCharCode(97 + index).repeat(64));
  enter("Node name", "Interactive Node");
  if (purpose === "create") { enter("Administrator name", "Alice"); enter("IdP administrator password", "protected-password"); }
  else enter("Trusted source", "https://source.test/management#" + "d".repeat(64));
  target.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })); flushSync();
  expect([...target.querySelectorAll<HTMLInputElement>('input[type="password"]')].every(node => node.value === "")).toBe(true);
  await vi.waitFor(() => expect(fetchApi.mock.calls.filter(([url]) => url.endsWith("/" + purpose))).toHaveLength(3));
  const requests = fetchApi.mock.calls.filter(([url]) => url.endsWith("/" + purpose));
  for (let index = 0; index < requests.length; index++) {
    const [url, init] = requests[index];
    expect(url).toBe(bases[owners[index]] + "/setup/" + purpose);
    expect((init.headers as Record<string, string>).Authorization).toBe("Setup " + String.fromCharCode(97 + index).repeat(64));
    const body = JSON.parse(init.body as string);
    expect(body.administrator).toBe(purpose === "create" ? "Alice" : "");
    expect("password" in body).toBe(purpose === "create" && index === 0);
  }
  expect(target.querySelector('a[href="/signin"]')).toBeNull();
  ready = true;
  await vi.waitFor(() => expect(button("Observe all owners").disabled).toBe(false));
  button("Observe all owners").click(); flushSync();
  await vi.waitFor(() => expect(target.querySelector('a[href="/signin"]')).not.toBeNull());
  expect(target.textContent).toContain("same exact operation and issuer");
});
