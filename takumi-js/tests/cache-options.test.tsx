import { expect, test } from "bun:test";
import { cacheStats, render } from "../src";

test("cacheStats reports the shared renderer after its first render", async () => {
  await render(<div style={{ width: 10, height: 10 }} className="box" />, {
    width: 10,
    height: 10,
    css: ".box { background: rgb(255, 0, 0); }",
  });

  expect(cacheStats()?.memory.current).toBeGreaterThan(0);
});
