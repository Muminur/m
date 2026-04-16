import { test, expect } from "@playwright/test";

test.describe("M3 Transcription", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/transcribe");
  });

  test("DropZone renders with upload prompt", async ({ page }) => {
    await expect(page.getByRole("heading", { name: "Transcribe" })).toBeVisible();
    await expect(
      page.getByText(/Drop an audio file or click to select/)
    ).toBeVisible();
  });

  test("DropZone shows accepted file formats", async ({ page }) => {
    await expect(page.getByText(/MP3, WAV, M4A, FLAC, OGG/)).toBeVisible();
  });

  test("YouTube import section is present", async ({ page }) => {
    await expect(page.getByText(/Import from YouTube/)).toBeVisible();
    await expect(
      page.getByPlaceholder(/youtube\.com/)
    ).toBeVisible();
  });

  test("Transcribe button is present and disabled without file", async ({ page }) => {
    const transcribeButton = page.locator("button", { hasText: "Transcribe" });
    await expect(transcribeButton).toBeVisible();
    await expect(transcribeButton).toBeDisabled();
  });

  test("browse link is present in drop area", async ({ page }) => {
    await expect(page.getByText("browse")).toBeVisible();
  });
});
