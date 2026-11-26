import { createRawSnippet, flushSync, mount, unmount } from "svelte";
import { expect, it } from "vitest";
import SecretDetail from "./SecretDetail.svelte";

it("labels missing and invalid timestamps without an invalid semantic time", async () => {
  const target = document.createElement("div");
  const component = mount(SecretDetail, {
    target,
    props: {
      record: {
        id: "00000000-0000-4000-8000-000000000001",
        kind: "login",
        name: "Synthetic login",
        favorite: false,
        uris: [],
        fields: [],
        history: [{ secret: "synthetic-password", changedAt: "2025-02-30T00:00:00Z" }],
        createdAt: "",
        updatedAt: "invalid",
      },
      canEdit: true,
      onedit: () => {},
      onclose: () => {},
      folderAssignment: createRawSnippet(() => ({ render: () => "<div></div>" })),
    },
  });
  try {
    flushSync();
    expect([...target.querySelectorAll("p")].map((element) => element.textContent)).toEqual(
      expect.arrayContaining([
        "Created: Unknown",
        "Updated: Unknown",
        "Password changed at Unknown",
      ])
    );
    expect(target.querySelector("time")).toBeNull();
  } finally {
    await unmount(component);
  }
});
