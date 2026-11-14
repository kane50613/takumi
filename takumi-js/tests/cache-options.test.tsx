import { afterAll, expect, test } from "bun:test";
import { cacheStats, render, setCacheOptions } from "../src";

afterAll(() => setCacheOptions({}));

test("setCacheOptions applies to the shared renderer after its first render", async () => {
  await render(<div style={{ width: 10, height: 10 }} className="box" />, {
    width: 10,
    height: 10,
    css: ".box { background: rgb(255, 0, 0); }",
  });

  expect(cacheStats()?.memory.current).toBeGreaterThan(0);

  setCacheOptions({ maxBytes: 0 });

  expect(cacheStats()?.memory).toMatchObject({ current: 0, max: 0 });
});
