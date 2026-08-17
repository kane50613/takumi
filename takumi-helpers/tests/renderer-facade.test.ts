import { describe, expect, mock, spyOn, test } from "bun:test";
import { FontRegistry, prepareRenderInput } from "../src/renderer";
import type { Font, FontLoader } from "../src/renderer";
import type { Node } from "../src/types";

const bytes = () => new Uint8Array([1, 2, 3]);
const textNode = (text: string): Node => ({ type: "text", text });

const rangedFont = (name: string, ranges: [number, number][]): FontLoader => ({
  name,
  ranges,
  data: () => bytes(),
});

const registry = () => {
  const registerInner = mock((font: Font) => [
    { name: "name" in font && font.name ? font.name : "Fam" },
  ]);
  return { registerInner, fonts: new FontRegistry(registerInner) };
};

/** Mirrors the per-backend `Renderer` wiring so the shared body is exercised through all methods. */
const stub = () => {
  const { registerInner, fonts } = registry();
  const inner = {
    render: mock((_node: Node, _options: unknown, _signal?: AbortSignal) => "png"),
    renderSvg: mock((_node: Node, _options: unknown, _signal?: AbortSignal) => "svg"),
    measure: mock((_node: Node, _options: unknown, _signal?: AbortSignal) => "measured"),
    renderAnimation: mock((_options: unknown, _signal?: AbortSignal) => "anim"),
  };
  return {
    inner,
    registerInner,
    async render(node: Node, options?: { signal?: AbortSignal; fonts?: FontLoader[] }) {
      const { options: opts, signal } = await prepareRenderInput(fonts, options ?? {}, node);
      return inner.render(node, opts, signal);
    },
    async renderSvg(node: Node, options?: { signal?: AbortSignal; fonts?: FontLoader[] }) {
      const { options: opts, signal } = await prepareRenderInput(fonts, options ?? {}, node);
      return inner.renderSvg(node, opts, signal);
    },
    async measure(node: Node, options?: { signal?: AbortSignal; fonts?: FontLoader[] }) {
      const { options: opts, signal } = await prepareRenderInput(fonts, options ?? {}, node);
      return inner.measure(node, opts, signal);
    },
    async renderAnimation(options: {
      scenes: { node: Node }[];
      signal?: AbortSignal;
      fonts?: FontLoader[];
    }) {
      const nodes = options.scenes.map((scene) => scene.node);
      const { options: opts, signal } = await prepareRenderInput(fonts, options, nodes);
      return inner.renderAnimation(opts, signal);
    },
  };
};

describe("prepareRenderInput abort policy", () => {
  test("a pre-aborted signal rejects before resources resolve", async () => {
    const { fonts } = registry();
    const resolve = spyOn(fonts, "resolveResources");

    await expect(
      prepareRenderInput(fonts, { signal: AbortSignal.abort() }, textNode("A")),
    ).rejects.toThrow();
    expect(resolve).not.toHaveBeenCalled();
  });

  test("a signal aborted during resolution rejects after resources resolve", async () => {
    const { fonts } = registry();
    const resolve = spyOn(fonts, "resolveResources");
    const controller = new AbortController();
    const font: FontLoader = {
      name: "Fam",
      ranges: [[0x41, 0x5a]],
      data: () => {
        controller.abort();
        return bytes();
      },
    };

    await expect(
      prepareRenderInput(fonts, { fonts: [font], signal: controller.signal }, textNode("A")),
    ).rejects.toThrow();
    expect(resolve).toHaveBeenCalled();
  });
});

describe("prepareRenderInput resource forwarding", () => {
  test("fonts, images and fontFamilies reach resolveResources", async () => {
    const { fonts } = registry();
    const resolve = spyOn(fonts, "resolveResources");
    const font = rangedFont("Fam", [[0x41, 0x5a]]);
    const images = [{ src: "a", data: bytes() }];

    await prepareRenderInput(
      fonts,
      { fonts: [font], images, fontFamilies: ["Explicit"] },
      textNode("A"),
    );

    expect(resolve).toHaveBeenCalledWith([font], images, ["Explicit"]);
  });

  test("fonts accepts a promise of the list", async () => {
    const { fonts } = registry();
    const resolve = spyOn(fonts, "resolveResources");
    const font = rangedFont("Fam", [[0x41, 0x5a]]);

    await prepareRenderInput(fonts, { fonts: Promise.resolve([font]) }, textNode("A"));

    expect(resolve).toHaveBeenCalledWith([font], undefined, undefined);
  });

  test("source drives font subsetting", async () => {
    const { fonts } = registry();
    const resolve = spyOn(fonts, "resolveResources");
    const font = rangedFont("Fam", [[0x4e00, 0x4e00]]);

    await prepareRenderInput(fonts, { fonts: [font] }, textNode("0"));

    expect(resolve).toHaveBeenCalledWith([], undefined, undefined);
  });

  test("a face covering list-marker characters stays without list content", async () => {
    const { fonts } = registry();
    const resolve = spyOn(fonts, "resolveResources");
    const font = rangedFont("Fam", [[0x2022, 0x2022]]);

    await prepareRenderInput(fonts, { fonts: [font] }, textNode("一"));

    expect(resolve).toHaveBeenCalledWith([font], undefined, undefined);
  });
});

describe("shared Renderer body", () => {
  test("every method forwards resolved resources to its inner call", async () => {
    const backend = stub();
    const font = rangedFont("Fam", [[0x41, 0x5a]]);
    const node = textNode("A");

    await backend.render(node, { fonts: [font] });
    await backend.renderSvg(node, { fonts: [font] });
    await backend.measure(node, { fonts: [font] });

    for (const call of [backend.inner.render, backend.inner.renderSvg, backend.inner.measure]) {
      expect(call).toHaveBeenCalledWith(node, { fontFamilies: ["Fam"] }, undefined);
    }
  });

  test("a pre-aborted signal keeps the inner call from running", async () => {
    const backend = stub();

    await expect(backend.render(textNode("A"), { signal: AbortSignal.abort() })).rejects.toThrow();
    expect(backend.inner.render).not.toHaveBeenCalled();
  });

  test("renderAnimation subsets fonts across every scene node", async () => {
    const backend = stub();
    const fontA = rangedFont("A", [[0x41, 0x41]]);
    const fontB = rangedFont("B", [[0x42, 0x42]]);

    const scenes = [{ node: textNode("A") }, { node: textNode("B") }];
    await backend.renderAnimation({ scenes, fonts: [fontA, fontB] });

    expect(backend.inner.renderAnimation).toHaveBeenCalledWith(
      { scenes, fontFamilies: ["A", "B"] },
      undefined,
    );
  });
});
