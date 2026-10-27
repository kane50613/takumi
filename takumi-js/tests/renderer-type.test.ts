import { expect, test } from "bun:test";
import { render, type Renderer } from "../src";
import { Renderer as NativeRenderer } from "../src/node";
import { Renderer as WasmRenderer } from "../src/wasm";

test("either backend's renderer satisfies the exported Renderer type", async () => {
  const renderers: Renderer[] = [new NativeRenderer(), new WasmRenderer()];

  for (const renderer of renderers) {
    const png = await render(
      { type: "container", style: { width: 8, height: 8, backgroundColor: "red" }, children: [] },
      { width: 8, height: 8, renderer },
    );

    expect(Array.from(png.subarray(1, 4))).toEqual([0x50, 0x4e, 0x47]);
  }
});
