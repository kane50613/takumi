import { expect, test } from "bun:test";
import { cssVariables } from "../src/css-variables";

test("joins nested keys into one variable name", () => {
  expect(cssVariables({ color: { brand: { 500: "#5b21b6" } }, spacing: "0.25rem" })).toEqual({
    "--color-brand-500": "#5b21b6",
    "--spacing": "0.25rem",
  });
});

test("stringifies numeric leaves", () => {
  expect(cssVariables({ leading: { tight: 1.25 } })).toEqual({ "--leading-tight": "1.25" });
});
