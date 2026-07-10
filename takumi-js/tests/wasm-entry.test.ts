import { describe, expect, test } from "bun:test";
import { render, renderSvg } from "../src/wasm";
import type { Node } from "@takumi-rs/helpers";

const node: Node = {
  type: "container",
  width: 8,
  height: 8,
  children: [{ type: "text", text: "wasm" }],
};

// On Bun the main entry's `#backend` condition picks napi, so output here
// proves the entry actually forced the WASM backend.
describe("takumi-js/wasm", () => {
  test("render() uses the bundled WASM binary", async () => {
    const png = await render(node, { width: 8, height: 8 });

    expect(png.subarray(0, 4)).toEqual(new Uint8Array([0x89, 0x50, 0x4e, 0x47]));
  });

  test("renderSvg() shares the managed WASM renderer", async () => {
    const svg = await renderSvg(node, { width: 8, height: 8 });

    expect(svg).toStartWith("<svg");
  });

  test("does not poison the main entry's backend slot", async () => {
    const { render: renderAuto } = await import("../src");
    const png = await renderAuto(node, { width: 8, height: 8 });

    expect(png.subarray(0, 4)).toEqual(new Uint8Array([0x89, 0x50, 0x4e, 0x47]));
  });
});
