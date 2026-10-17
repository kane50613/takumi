import { describe, expect, mock, spyOn, test } from "bun:test";
import { withRenderInput } from "../src/renderer";
import type { FontLoader } from "../src/renderer";
import type { Node } from "../src/types";
import { fontBackend } from "./font-backend";

const bytes = () => new Uint8Array([1, 2, 3]);
const textNode = (text: string): Node => ({ type: "text", text });

const rangedFont = (name: string, unicodeRange: [number, number][]): FontLoader => ({
  name,
  unicodeRange,
  data: () => bytes(),
});

/** Runs {@link withRenderInput} with a render that does nothing. */
const prepare = (
  fonts: ReturnType<typeof fontBackend>["fonts"],
  options: Parameters<typeof withRenderInput>[1],
  source: Node,
) => withRenderInput(fonts, options, source, () => undefined);

/** Mirrors the per-backend `Renderer` wiring so the shared body is exercised through all methods. */
const stub = () => {
  const { fonts } = fontBackend();
  const inner = {
    render: mock((_node: Node, _options: unknown, _signal?: AbortSignal) => "png"),
    renderSvg: mock((_node: Node, _options: unknown, _signal?: AbortSignal) => "svg"),
    measure: mock((_node: Node, _options: unknown, _signal?: AbortSignal) => "measured"),
    renderAnimation: mock((_options: unknown, _signal?: AbortSignal) => "anim"),
  };
  return {
    inner,
    render(node: Node, options?: { signal?: AbortSignal; fonts?: FontLoader[] }) {
      return withRenderInput(fonts, options ?? {}, node, (opts, signal) =>
        inner.render(node, opts, signal),
      );
    },
    renderSvg(node: Node, options?: { signal?: AbortSignal; fonts?: FontLoader[] }) {
      return withRenderInput(fonts, options ?? {}, node, (opts, signal) =>
        inner.renderSvg(node, opts, signal),
      );
    },
    measure(node: Node, options?: { signal?: AbortSignal; fonts?: FontLoader[] }) {
      return withRenderInput(fonts, options ?? {}, node, (opts, signal) =>
        inner.measure(node, opts, signal),
      );
    },
    renderAnimation(options: {
      scenes: { node: Node }[];
      signal?: AbortSignal;
      fonts?: FontLoader[];
    }) {
      const nodes = options.scenes.map((scene) => scene.node);
      return withRenderInput(fonts, options, nodes, (opts, signal) =>
        inner.renderAnimation(opts, signal),
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

describe("withRenderInput abort policy", () => {
  test("a pre-aborted signal rejects before resources resolve", async () => {
    const { fonts } = fontBackend();
    const resolve = spyOn(fonts, "resolveResources");

    await expect(prepare(fonts, { signal: AbortSignal.abort() }, textNode("A"))).rejects.toThrow();
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
      prepare(fonts, { fonts: [font], signal: controller.signal }, textNode("A")),
    ).rejects.toThrow();
    expect(resolve).toHaveBeenCalled();
  });
});

describe("withRenderInput resource forwarding", () => {
  test("fonts, images and fontFamilies reach resolveResources", async () => {
    const { fonts } = fontBackend();
    const resolve = spyOn(fonts, "resolveResources");
    const font = rangedFont("Fam", [[0x41, 0x5a]]);
    const images = [{ src: "a", data: bytes() }];

    await prepare(fonts, { fonts: [font], images, fontFamilies: ["Explicit"] }, textNode("A"));

    expect(resolve).toHaveBeenCalledWith([font], images, ["Explicit"], expect.any(Function));
  });

  test("fonts accepts a promise of the list", async () => {
    const { fonts } = fontBackend();
    const resolve = spyOn(fonts, "resolveResources");
    const font = rangedFont("Fam", [[0x41, 0x5a]]);

    await prepare(fonts, { fonts: Promise.resolve([font]) }, textNode("A"));

    expect(resolve).toHaveBeenCalledWith([font], undefined, undefined, expect.any(Function));
  });

  test("source drives font subsetting", async () => {
    const { fonts } = fontBackend();
    const resolve = spyOn(fonts, "resolveResources");
    const font = rangedFont("Fam", [[0x4e00, 0x4e00]]);

    await prepare(fonts, { fonts: [font] }, textNode("0"));

    expect(resolve).toHaveBeenCalledWith([], undefined, undefined, expect.any(Function));
  });

  test("a face covering list-marker characters stays without list content", async () => {
    const { fonts } = fontBackend();
    const resolve = spyOn(fonts, "resolveResources");
    const font = rangedFont("Fam", [[0x2022, 0x2022]]);

    await prepare(fonts, { fonts: [font] }, textNode("一"));

    expect(resolve).toHaveBeenCalledWith([font], undefined, undefined, expect.any(Function));
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
