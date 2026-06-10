import { describe, expect, test } from "bun:test";
import { shouldLoadDefaultFonts } from "../src/renderer";

const font = { name: "Geist", data: new Uint8Array(), weight: 400 };

describe("shouldLoadDefaultFonts", () => {
  test("defers to the renderer default when no fonts are provided", () => {
    expect(shouldLoadDefaultFonts(undefined)).toBeUndefined();
    expect(shouldLoadDefaultFonts({})).toBeUndefined();
    expect(shouldLoadDefaultFonts({ fonts: [] })).toBeUndefined();
  });

  test("disables default fonts when custom fonts are provided", () => {
    expect(shouldLoadDefaultFonts({ fonts: [font] })).toBe(false);
  });

  test("respects an explicit loadDefaultFonts option", () => {
    expect(shouldLoadDefaultFonts({ fonts: [font], loadDefaultFonts: true })).toBe(true);
    expect(shouldLoadDefaultFonts({ loadDefaultFonts: false })).toBe(false);
  });
});
