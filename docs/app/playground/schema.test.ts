import { expect, test } from "bun:test";
import { optionsSchema } from "./schema";

function accepts(css: unknown): boolean {
  return optionsSchema.safeParse({ width: 1, height: 1, css }).success;
}

test("takes one entry or a list of them", () => {
  expect(accepts(undefined)).toBe(true);
  expect(accepts("div{color:red}")).toBe(true);
  expect(accepts(["a{}", { selector: ".b" }])).toBe(true);
});

test("takes every rule shape, nested", () => {
  expect(accepts({ selector: ":root", style: { "--a": "1px", "--b": 2 } })).toBe(true);
  expect(accepts({ selector: ".a", rules: [{ selector: "&:hover" }] })).toBe(true);
  expect(accepts({ keyframes: "spin", steps: [{ offset: "from", style: { opacity: "0" } }] })).toBe(
    true,
  );
  expect(accepts({ media: "(min-width: 800px)", rules: [{ selector: ".a" }] })).toBe(true);
  expect(accepts({ supports: "(display: grid)", rules: ["a{}"] })).toBe(true);
  expect(accepts({ layer: "base" })).toBe(true);
});

test("rejects a value that is not an entry", () => {
  expect(accepts(null)).toBe(false);
  expect(accepts(42)).toBe(false);
  expect(accepts(["a{}", null])).toBe(false);
  expect(accepts({ style: { color: "red" } })).toBe(false);
});

/// A stripped key would leave a rule the renderer never sees, so a typo has to
/// fail here rather than render as an empty rule.
test("rejects an unknown key", () => {
  expect(accepts({ selector: ".a", declarations: { color: "red" } })).toBe(false);
  expect(accepts({ selector: ".a", rules: [{ selector: ".b", declarations: {} }] })).toBe(false);
  expect(accepts({ keyframes: "spin", steps: [{ offset: "from", declarations: {} }] })).toBe(false);
  expect(accepts({ media: "print", entries: [] })).toBe(false);
});
