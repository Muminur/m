import "@testing-library/jest-dom";
import { vi } from "vitest";

// Global mocks for Tauri APIs — provides safe defaults so individual tests
// don't need to mock these unless they need custom behavior.
// Per-file vi.mock() calls override these when present.

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));
