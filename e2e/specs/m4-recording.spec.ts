import { test, expect } from "@playwright/test";

test.describe("M4 Recording", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/recording");
  });

  test("recording panel renders with heading", async ({ page }) => {
    await expect(page.getByRole("heading", { name: "Recording" })).toBeVisible();
    await expect(
      page.getByText(/Capture audio from microphone or system audio/)
    ).toBeVisible();
  });

  test("audio source selector buttons are visible", async ({ page }) => {
    const main = page.getByRole("main");
    await expect(main.getByRole("button", { name: /Microphone/ })).toBeVisible();
    await expect(main.getByRole("button", { name: /System/ })).toBeVisible();
    await expect(main.getByRole("button", { name: /Both/ })).toBeVisible();
  });

  test("Start Recording button is visible in idle state", async ({ page }) => {
    await expect(page.getByRole("button", { name: /Start Recording/ })).toBeVisible();
  });

  test("VU meter level indicator is present", async ({ page }) => {
    await expect(page.getByText("Level")).toBeVisible();
  });

  test("timer displays initial 00:00", async ({ page }) => {
    await expect(page.getByText("00:00")).toBeVisible();
  });
});
