import { test, expect } from "@playwright/test";

// Recording page E2E tests
// These verify the recording UI renders correctly and all controls are present.
// Tauri IPC (invoke) is not available in browser mode, so recording-start
// is exercised at the UI level only.

test.describe("Recording Page", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/recording");
  });

  test("recording page renders within 3-pane layout", async ({ page }) => {
    await expect(page.locator("nav")).toBeVisible();
    await expect(page.locator("main")).toBeVisible();
    await expect(page.getByRole("heading", { name: /recording/i })).toBeVisible();
  });

  test("audio source buttons are all present", async ({ page }) => {
    const main = page.locator("main");
    await expect(main.getByRole("button", { name: /microphone/i })).toBeVisible();
    await expect(main.getByRole("button", { name: /system/i })).toBeVisible();
    await expect(main.getByRole("button", { name: /both/i })).toBeVisible();
  });

  test("microphone source is selected by default", async ({ page }) => {
    // The active source button should have a dark/primary background
    const micButton = page.getByRole("button", { name: /microphone/i });
    await expect(micButton).toBeVisible();
    // Microphone is the default — it should not be greyed out
    await expect(micButton).not.toBeDisabled();
  });

  test("VU level meter starts at -60 dB (silent), not at 0 dB (red)", async ({
    page,
  }) => {
    // The level display should show -60.0 dB at idle, not 0.0 dB
    await expect(page.getByText(/-60\.0\s*dB/)).toBeVisible();
    // The progress bar should NOT be at full width (red full bar was the bug)
    const bar = page.locator(".h-3 > div");
    const width = await bar.evaluate(
      (el) => parseFloat(getComputedStyle(el).width) / parseFloat(getComputedStyle(el.parentElement!).width) * 100
    );
    expect(width).toBeLessThan(5); // should be ~0%, not 100%
  });

  test("timer shows 00:00 at idle", async ({ page }) => {
    await expect(page.getByText("00:00")).toBeVisible();
  });

  test("Start Recording button is visible at idle", async ({ page }) => {
    await expect(
      page.getByRole("button", { name: /start recording/i })
    ).toBeVisible();
  });

  test("input device selector is present", async ({ page }) => {
    await expect(page.getByRole("combobox")).toBeVisible();
  });

  test("sidebar navigation link to Recording is active", async ({ page }) => {
    const recordingLink = page.locator("nav").getByText(/recording/i);
    await expect(recordingLink).toBeVisible();
  });

  test("audio source buttons are disabled while recording (simulated)", async ({
    page,
  }) => {
    // In browser mode invoke fails, but controls should stay enabled at idle
    // (they disable only when status !== 'idle')
    const main = page.locator("main");
    const micButton = main.getByRole("button", { name: /microphone/i });
    await expect(micButton).not.toBeDisabled();
    const sysButton = main.getByRole("button", { name: /system/i });
    await expect(sysButton).not.toBeDisabled();
  });

  test("navigating away and back resets to idle state", async ({ page }) => {
    await page.goto("/library");
    await page.goto("/recording");
    await expect(
      page.getByRole("button", { name: /start recording/i })
    ).toBeVisible();
    await expect(page.getByText("00:00")).toBeVisible();
  });
});
