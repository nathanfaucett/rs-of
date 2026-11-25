import { createRawSnippet, flushSync, mount, unmount } from "svelte";
import { afterEach, expect, it, vi } from "vitest";
import { Field, type FieldControl } from "$lib/ui";
import PasswordInput from "./PasswordInput.svelte";
import ProtectedValue from "./ProtectedValue.svelte";

const secret = "synthetic-private-value";
const target = document.createElement("div");
let components: ReturnType<typeof mount>[] = [];

afterEach(async () => {
  for (const component of components) await unmount(component);
  components = [];
  target.remove();
  vi.restoreAllMocks();
});

function field(id: string, label: string) {
  components.push(
    mount(Field, {
      target,
      props: {
        id,
        label,
        required: true,
        description: `Help for ${label}`,
        issues: [{ message: `Issue for ${label}` }],
        children: createRawSnippet<[FieldControl]>((control) => ({
          render: () => "<div></div>",
          setup: (element) => {
            const input = mount(PasswordInput, {
              target: element,
              props: { ...control(), label, value: secret },
            });
            return () => {
              void unmount(input);
            };
          },
        })),
      },
    })
  );
}

function button(name: string) {
  const result = [...target.querySelectorAll("button")].find(
    (element) => (element.getAttribute("aria-label") ?? element.textContent?.trim()) === name
  );
  if (!result) throw new Error(`Missing button: ${name}`);
  return result;
}

it("gives each protected control an exact label, unique descriptions, and independent reveal", () => {
  document.body.append(target);
  field("first", "Password");
  field("second", "Confirm password");
  flushSync();
  const inputs = [...target.querySelectorAll("input")];
  expect(inputs.map((input) => input.labels?.[0]?.textContent?.trim())).toEqual([
    "Password",
    "Confirm password",
  ]);
  const ids = inputs.flatMap((input) => input.getAttribute("aria-describedby")?.split(" ") ?? []);
  expect(new Set(ids).size).toBe(4);
  expect(ids.map((id) => document.getElementById(id)?.textContent?.trim())).toEqual([
    "Help for Password",
    "Issue for Password",
    "Help for Confirm password",
    "Issue for Confirm password",
  ]);
  expect(inputs.map((input) => input.type)).toEqual(["password", "password"]);
  expect(button("Show Password").getAttribute("aria-pressed")).toBe("false");
  button("Show Password").click();
  flushSync();
  expect(inputs.map((input) => input.type)).toEqual(["text", "password"]);
  expect(button("Hide Password").getAttribute("aria-pressed")).toBe("true");
});

it("reports clipboard denial and success without leaking the value or exception", async () => {
  document.body.append(target);
  field("password", "Password");
  components.push(
    mount(ProtectedValue, { target, props: { label: "History password", value: secret } })
  );
  flushSync();
  const write = vi
    .spyOn(navigator.clipboard, "writeText")
    .mockRejectedValueOnce(new Error("PRIVATE-CLIPBOARD-ERROR"))
    .mockResolvedValueOnce();

  button("Copy Password").click();
  await expect
    .poll(() => target.querySelector('[role="alert"]')?.textContent)
    .toContain("Copy failed.");
  button("Copy History password").click();
  await expect
    .poll(() => target.querySelector('[role="status"]')?.textContent)
    .toContain("Copied.");
  expect(write.mock.calls).toEqual([[secret], [secret]]);
  expect(target.textContent).not.toContain(secret);
  expect(target.textContent).not.toContain("PRIVATE-CLIPBOARD-ERROR");
});
