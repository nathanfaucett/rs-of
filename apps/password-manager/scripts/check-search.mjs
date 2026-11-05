import assert from "node:assert/strict";

export async function checkSearch(page) {
    const results = page.getByRole("list", { name: "Search results" });
    const search = page.getByRole("searchbox", { name: "Search items" });
    const before = await page.evaluate(async () => ({
        local: JSON.stringify(localStorage),
        session: JSON.stringify(sessionStorage),
        databases: await indexedDB.databases(),
        url: location.href,
    }));
    await search.focus();
    await search.pressSequentially("SYNTHETIC-USER-19");
    await page.getByLabel("Item type").selectOption("login");
    await page.getByLabel("Sort items").selectOption("updatedDesc");
    await results
        .getByRole("button", { name: "Synthetic offline Login", exact: true })
        .waitFor();
    await page.getByLabel("Favorites only").focus();
    await page.getByLabel("Favorites only").press("Space");
    await page
        .getByText("No items match. Clear filters to show all items.", {
            exact: true,
        })
        .waitFor();
    await page
        .getByRole("button", { name: "Clear filters", exact: true })
        .press("Enter");
    assert.equal(await search.inputValue(), "");
    assert.equal(await page.getByLabel("Item type").inputValue(), "all");
    assert.equal(await page.getByLabel("Favorites only").isChecked(), false);
    await results
        .getByRole("button", { name: "Synthetic offline Login", exact: true })
        .press("Enter");
    assert.equal(await page.locator("form").count(), 0);
    await search.fill("Synthetic offline Login");
    assert.deepEqual(
        await page.evaluate(async () => ({
            local: JSON.stringify(localStorage),
            session: JSON.stringify(sessionStorage),
            databases: await indexedDB.databases(),
            url: location.href,
        })),
        before,
    );
    console.log(
        "Offline search/filter/sort: keyboard controls, read-only result, unchanged UI storage passed",
    );
}
