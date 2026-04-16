import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen } from "@tauri-apps/api/event";
import type { EventCallback, UnlistenFn } from "@tauri-apps/api/event";

/**
 * Check whether we're running inside the Tauri webview.
 * In plain browser (dev server, tests) window.__TAURI_INTERNALS__ is undefined.
 */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/**
 * Call a Tauri command, returning `undefined` if not in Tauri.
 */
export async function safeInvoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T | undefined> {
  if (!isTauri()) return undefined;
  return tauriInvoke<T>(cmd, args);
}

/**
 * Listen for a Tauri event. Returns a no-op unlistener if not in Tauri.
 */
export function safeListen<T>(
  event: string,
  handler: EventCallback<T>,
): Promise<UnlistenFn> {
  if (!isTauri()) return Promise.resolve(() => {});
  return tauriListen<T>(event, handler);
}
