import { expect, mock, test } from "bun:test";

const binary = new Uint8Array([0, 97, 115, 109]);
const init = mock(async (_input?: { module_or_path: unknown }) => {});

// Stands in for the bindings, so the test checks how often they initialize without compiling them.
mock.module("@takumi-rs/wasm", () => ({ init, Renderer: class {}, setGlyphCacheMaxBytes() {} }));

const { initWasm } = await import("../src/backend/wasm-init");

test("initWasm shares one load between overlapping calls, and loads again after a failure", async () => {
  init.mockRejectedValueOnce(new Error("compile failed"));

  await expect(initWasm(binary)).rejects.toThrow("compile failed");

  await Promise.all([initWasm(binary), initWasm(binary), initWasm(new Uint8Array([1]))]);
  await initWasm(binary);

  expect(init).toHaveBeenCalledTimes(2);
  expect(init).toHaveBeenLastCalledWith({ module_or_path: binary });
});
