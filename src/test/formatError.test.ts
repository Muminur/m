import { describe, expect, it } from "vitest";
import { formatError } from "@/lib/formatError";

describe("formatError", () => {
  it("extracts the message from a structured Tauri AppError", () => {
    expect(
      formatError({
        kind: "TranscriptionError",
        detail: {
          code: "InferenceFailure",
          message: "A transcription job is already running",
        },
      })
    ).toBe("A transcription job is already running");
  });

  it("preserves normal Error and string messages", () => {
    expect(formatError(new Error("boom"))).toBe("boom");
    expect(formatError("offline")).toBe("offline");
  });

  it("serializes an otherwise unknown object", () => {
    expect(formatError({ code: 42 })).toBe('{"code":42}');
  });
});
