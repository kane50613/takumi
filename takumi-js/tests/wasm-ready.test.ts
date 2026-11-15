import { expect, mock, test } from "bun:test";

const binary = new Uint8Array([0, 97, 115, 109]);
const init = mock(async (_input?: { module_or_path: unknown }) => {});

// Stands in for the bindings, so the test checks how often they initialize without compiling them.
mock.module("@takumi-rs/wasm", () => ({ init, Renderer: class {}, setGlyphCacheMaxBytes() {} }));
mock.module("@takumi-rs/wasm/auto", () => ({ default: Promise.resolve(binary) }));

const { ready } = await import("../src/wasm");
const { initWasm } = await import("../src/backend/wasm-init");

test("ready initializes the bindings once from the binary the conditions pick", async () => {
  init.mockRejectedValueOnce(new Error("compile failed"));

  await expect(ready()).rejects.toThrow("compile failed");

  const overlapping = [ready(), ready(), initWasm(new Uint8Array([1]))];

  await Promise.all(overlapping);
  await ready();

  expect(init).toHaveBeenCalledTimes(2);
  expect(init).toHaveBeenLastCalledWith({ module_or_path: binary });
});
