import { describe, expect, mock, spyOn, test } from "bun:test";
import { readSource, renderPrepared } from "../src/renderer";
import type { FontLoader, PrepareExtras } from "../src/renderer";
import type { Node } from "../src/types";
import { fontBackend } from "./font-backend";

const bytes = () => new Uint8Array([1, 2, 3]);
const textNode = (text: string): Node => ({ type: "text", text });

const rangedFont = (name: string, unicodeRange: [number, number][]): FontLoader => ({
  name,
  unicodeRange,
  data: () => bytes(),
});

/** Reads `node`, whose text stands in for the characters a backend reads, then renders nothing. */
const render = async (
  fonts: ReturnType<typeof fontBackend>["fonts"],
  extras: PrepareExtras & { fontFamilies?: string[] },
  node: Node,
) =>
  renderPrepared(
    fonts,
    await readSource(node, {}, extras, () => ({
      images: [],
      characters: node.type === "text" ? node.text : "",
    })),
    { signal: extras.signal },
    () => undefined,
  );

/** Mirrors the per-backend `Renderer` wiring so the shared body is exercised through all methods. */
const stub = () => {
  const { fonts } = fontBackend();
  const inner = {
    render: mock((_node: Node, _options: unknown, _signal?: AbortSignal) => "png"),
    renderSvg: mock((_node: Node, _options: unknown, _signal?: AbortSignal) => "svg"),
    measure: mock((_node: Node, _options: unknown, _signal?: AbortSignal) => "measured"),
    renderAnimation: mock((_options: unknown, _signal?: AbortSignal) => "anim"),
  };
  const characters = (nodes: Node[]) =>
    nodes.map((node) => (node.type === "text" ? node.text : "")).join("");
  const draw =
    (call: typeof inner.render) =>
    async (node: Node, options: { signal?: AbortSignal; fonts?: FontLoader[] } = {}) =>
      renderPrepared(
        fonts,
        await readSource(node, {}, options, () => ({ images: [], characters: characters([node]) })),
        { signal: options.signal },
        (source, opts, signal) => call(source, opts, signal),
      );

  return {
    inner,
    render: draw(inner.render),
    renderSvg: draw(inner.renderSvg),
    measure: draw(inner.measure),
    async renderAnimation(options: {
      scenes: { node: Node }[];
      signal?: AbortSignal;
      fonts?: FontLoader[];
    }) {
      const { scenes, ...extras } = options;
      const prepared = await readSource(scenes, {}, extras, () => ({
        images: [],
        characters: characters(scenes.map((scene) => scene.node)),
      }));

      return renderPrepared(fonts, prepared, { signal: options.signal }, (sources, opts, signal) =>
        inner.renderAnimation({ ...opts, scenes: sources }, signal),
      );
    },
  };
};

describe("FontRegistry", () => {
  test("registers a deprecated ranges loader with its unicodeRange", async () => {
    const { backend, fonts } = fontBackend();

    await fonts.register({ name: "Fam", ranges: [[0x41, 0x5a]], data: () => bytes() });

    expect(backend.registerFont.mock.calls[0]?.[0]).toMatchObject({ unicodeRange: [[0x41, 0x5a]] });
  });
});

describe("readSource and renderPrepared abort policy", () => {
  test("a pre-aborted signal rejects before resources resolve", async () => {
    const { fonts } = fontBackend();
    const resolve = spyOn(fonts, "resolveResources");

    await expect(render(fonts, { signal: AbortSignal.abort() }, textNode("A"))).rejects.toThrow();
    expect(resolve).not.toHaveBeenCalled();
  });

  test("a signal aborted during resolution rejects after resources resolve", async () => {
    const { fonts } = fontBackend();
    const resolve = spyOn(fonts, "resolveResources");
    const controller = new AbortController();
    const font: FontLoader = {
      name: "Fam",
      unicodeRange: [[0x41, 0x5a]],
      data: () => {
        controller.abort();
        return bytes();
      },
    };

    await expect(
      render(fonts, { fonts: [font], signal: controller.signal }, textNode("A")),
    ).rejects.toThrow();
    expect(resolve).toHaveBeenCalled();
  });
});

describe("readSource and renderPrepared resource forwarding", () => {
  test("fonts, images and fontFamilies reach resolveResources", async () => {
    const { fonts } = fontBackend();
    const resolve = spyOn(fonts, "resolveResources");
    const font = rangedFont("Fam", [[0x41, 0x5a]]);
    const images = [{ src: "a", data: bytes() }];

    await render(fonts, { fonts: [font], images, fontFamilies: ["Explicit"] }, textNode("A"));

    expect(resolve).toHaveBeenCalledWith(
      [font],
      [{ src: "a", data: bytes(), cache: undefined }],
      ["Explicit"],
      expect.any(Function),
    );
  });

  test("fonts accepts a promise of the list", async () => {
    const { fonts } = fontBackend();
    const resolve = spyOn(fonts, "resolveResources");
    const font = rangedFont("Fam", [[0x41, 0x5a]]);

    await render(fonts, { fonts: Promise.resolve([font]) }, textNode("A"));

    expect(resolve).toHaveBeenCalledWith([font], [], undefined, expect.any(Function));
  });

  test("the characters the backend reads drive font subsetting", async () => {
    const { fonts } = fontBackend();
    const resolve = spyOn(fonts, "resolveResources");
    const font = rangedFont("Fam", [[0x4e00, 0x4e00]]);

    await render(fonts, { fonts: [font] }, textNode("0"));

    expect(resolve).toHaveBeenCalledWith([], [], undefined, expect.any(Function));
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
      expect(call).toHaveBeenCalledWith(node, { fontFamilies: ["Fam"], images: [] }, undefined);
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
      { scenes, fontFamilies: ["A", "B"], images: [] },
      undefined,
    );
  });
});
