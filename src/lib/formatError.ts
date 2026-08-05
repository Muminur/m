/**
 * Turn Tauri's structured command errors into something useful to a person.
 *
 * Rust `AppError` values arrive as objects shaped like
 * `{ kind, detail: { code, message } }`; `String(error)` reduces those to the
 * unhelpful "[object Object]".
 */
export function formatError(error: unknown): string {
  if (error instanceof Error && error.message) return error.message;
  if (typeof error === "string") return error;

  if (error && typeof error === "object") {
    const value = error as Record<string, unknown>;
    if (typeof value.message === "string") return value.message;
    if (typeof value.error === "string") return value.error;

    const detail = value.detail;
    if (typeof detail === "string") return detail;
    if (detail && typeof detail === "object") {
      const detailValue = detail as Record<string, unknown>;
      if (typeof detailValue.message === "string") return detailValue.message;
      if (typeof detailValue.error === "string") return detailValue.error;
    }

    try {
      const serialized = JSON.stringify(error);
      if (serialized && serialized !== "{}") return serialized;
    } catch {
      // Fall through to the generic message for cyclic/non-serializable data.
    }
  }

  return "Unknown error";
}
