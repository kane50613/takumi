import { afterAll, describe, expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { container, image } from "@takumi-rs/helpers";
import { Renderer } from "../bundlers/node";

const fontPath = join(
  import.meta.dir,
  "../../assets/fonts/manrope/manrope-latin-wght-normal.woff2",
);
const font = await readFile(fontPath);

const renderer = new Renderer();

afterAll(() => {
  renderer.free();
});

const recoveryNode = container({ style: { width: 8, height: 8, backgroundColor: "black" } });

const withImageBytes = (data: Uint8Array) =>
  container({
    style: { width: 64, height: 64 },
    children: [image({ src: data, width: 64, height: 64 })],
  });

// Undecodable image sources are skipped at paint like a browser's broken
// image, so renders complete instead of rejecting; these cases pin that no
// input traps the wasm instance and the renderer stays usable afterwards.
describe("malformed binary inputs", () => {
  const expectRecovery = async () => {
    const recovered = await renderer.render(recoveryNode, { width: 8, height: 8 });
    expect(recovered).toBeInstanceOf(Uint8Array);
  };

  test("corrupt PNG bytes render without crashing", async () => {
    const corrupt = new Uint8Array(72);
    corrupt.set([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
    for (let i = 8; i < corrupt.length; i++) {
      corrupt[i] = (i * 37) % 256;
    }

    const result = await renderer.render(withImageBytes(corrupt), { width: 64, height: 64 });
    expect(result).toBeInstanceOf(Uint8Array);
    await expectRecovery();
  });

  test("truncated font rejects", async () => {
    await expect(renderer.registerFont(font.subarray(0, 100))).rejects.toThrow();
    await expectRecovery();
  });

  test("malformed inline SVG bytes render without crashing", async () => {
    const bytes = new TextEncoder().encode("<svg garbage");

    const result = await renderer.render(withImageBytes(bytes), { width: 64, height: 64 });
    expect(result).toBeInstanceOf(Uint8Array);
    await expectRecovery();
  });

  test("truncated GIF bytes render without crashing", async () => {
    const truncated = new Uint8Array(16);
    truncated.set(new TextEncoder().encode("GIF89a"));

    const result = await renderer.render(withImageBytes(truncated), { width: 64, height: 64 });
    expect(result).toBeInstanceOf(Uint8Array);
    await expectRecovery();
  });
});
