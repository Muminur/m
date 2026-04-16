import { test, expect } from "@playwright/test";

test.describe("M5 Settings", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/settings");
  });

  test("settings page renders with heading", async ({ page }) => {
    await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
  });

  test("acceleration backend section is visible", async ({ page }) => {
    await expect(page.getByText("Acceleration Backend")).toBeVisible();
    await expect(page.getByText("Auto", { exact: true })).toBeVisible();
    await expect(page.getByText("CPU Only")).toBeVisible();
    await expect(page.getByText("Metal (GPU)")).toBeVisible();
  });

  test("API keys section is visible", async ({ page }) => {
    await expect(page.getByText("API Keys")).toBeVisible();
    await expect(page.getByText("OpenAI")).toBeVisible();
    await expect(page.getByText("Anthropic")).toBeVisible();
  });

  test("watch folders section is visible", async ({ page }) => {
    await expect(page.getByRole("heading", { name: "Watch Folders" })).toBeVisible();
  });

  test("navigation back from settings works", async ({ page }) => {
    await page.locator("nav").getByText(/library/i).click();
    await expect(page).toHaveURL(/\/library/);
  });
});
