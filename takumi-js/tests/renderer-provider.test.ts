import { expect, mock, test } from "bun:test";
import * as backend from "@takumi-rs/core";
import { RendererProvider } from "../src/backend/renderer";
import type { Backend } from "../src/backend/types";

test("coalesces loading and reuses one renderer per provider", async () => {
  const pending = Promise.withResolvers<Backend>();
  const load = mock(() => pending.promise);
  const provider = new RendererProvider(load);
  expect(load).not.toHaveBeenCalled();
  const first = provider.get();
  const second = provider.get();
  expect(load).toHaveBeenCalledTimes(1);
  pending.resolve(backend);
  expect(await first).toBe(await second);
  expect(await provider.get()).toBe(await first);
  expect(load).toHaveBeenCalledTimes(1);
});

test("independent providers do not share renderer instances", async () => {
  const load = mock(async () => backend);
  const first = new RendererProvider(load);
  const second = new RendererProvider(load);
  expect(await first.get()).not.toBe(await second.get());
  expect(load).toHaveBeenCalledTimes(2);
});

test("failed loading can retry with the requested module", async () => {
  const load = mock()
    .mockRejectedValueOnce(new Error("backend unavailable"))
    .mockResolvedValue(backend);
  const provider = new RendererProvider(load);
  await expect(provider.get()).rejects.toThrow("backend unavailable");
  const module = new Uint8Array([0, 97, 115, 109]);
  expect(await provider.get(module)).toBeInstanceOf(backend.Renderer);
  expect(load).toHaveBeenLastCalledWith(module);
  expect(load).toHaveBeenCalledTimes(2);
});

test("renderer construction can retry without reloading the backend", async () => {
  let constructions = 0;
  class RetryingRenderer extends backend.Renderer {
    constructor() {
      super();
      if (++constructions === 1) throw new Error("construction failed");
    }
  }
  const load = mock(async () => ({ ...backend, Renderer: RetryingRenderer }));
  const provider = new RendererProvider(load);
  await expect(provider.get()).rejects.toThrow("construction failed");
  expect(await provider.get()).toBeInstanceOf(RetryingRenderer);
  expect(load).toHaveBeenCalledTimes(1);
});
