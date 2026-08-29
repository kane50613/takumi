import { describe, expect, it } from "bun:test";
import { cssEntryToText } from "./preview-css";

describe("preview css", () => {
  it("rewrites a @theme block to :root", () => {
    expect(cssEntryToText("@theme inline { --color-brand: #f00; }")).toBe(
      ":root { --color-brand: #f00; }",
    );
  });
});
