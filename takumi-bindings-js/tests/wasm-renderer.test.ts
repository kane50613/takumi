import { expect, test } from "bun:test";
import { lazy } from "../src/wasm-renderer";

test("lazy creates its value once", () => {
  let made = 0;
  const value = lazy(() => ++made);

  expect([value(), value()]).toEqual([1, 1]);
});
