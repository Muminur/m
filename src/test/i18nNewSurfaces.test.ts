import { describe, expect, it } from "vitest";
import en from "@/i18n/en.json";
import nl from "@/i18n/nl.json";
import de from "@/i18n/de.json";

const newSurfaceSections = [
  "nav",
  "settings",
  "ai",
  "batch",
  "captions",
  "integrations",
  "watch_folders",
] as const;

describe("new-surface translations", () => {
  it("provides the same new-surface keys in English, Dutch, and German", () => {
    for (const section of newSurfaceSections) {
      expect(Object.keys(nl[section]).sort()).toEqual(Object.keys(en[section]).sort());
      expect(Object.keys(de[section]).sort()).toEqual(Object.keys(en[section]).sort());
    }
  });
});
