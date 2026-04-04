import { test, expect } from "@playwright/test";

// M1 Foundation E2E smoke tests
// These tests verify the basic shell renders correctly

test.describe("M1 Foundation", () => {
  test("app launches and renders main window", async ({ page }) => {
    await page.goto("/");
    // Should redirect to /library
    await expect(page).toHaveURL(/\/library/);
  });

  test("3-pane layout renders", async ({ page }) => {
    await page.goto("/library");
    // Sidebar should be present
    await expect(page.locator("nav")).toBeVisible();
    // Main content area should be present
    await expect(page.locator("main")).toBeVisible();
  });

  test("empty library state shows empty hint", async ({ page }) => {
    await page.goto("/library");
    // With no transcripts, empty state should show
    await expect(
      page.getByText(/No transcripts yet|Nog geen transcripties|Noch keine/i)
    ).toBeVisible({ timeout: 5000 });
  });

  test("theme toggle is accessible in sidebar", async ({ page }) => {
    await page.goto("/library");
    // Theme button should be present (Sun, Moon, or Monitor icon button)
    const themeButton = page.locator("button[title]").filter({ hasText: /light|dark|system/i });
    await expect(themeButton.first()).toBeVisible();
  });

  test("navigation items are present", async ({ page }) => {
    await page.goto("/library");
    const nav = page.locator("nav");
    await expect(nav.getByText(/library/i)).toBeVisible();
  });
});
