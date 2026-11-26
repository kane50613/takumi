// Type-checks the calls nuxt-og-image and Fumadocs make, copied from their sources, against the
// declarations the packages publish in `dist`, so it needs them built first.
import { readFile } from "node:fs/promises";
import { Renderer as CoreRenderer, type CssInput } from "@takumi-rs/core";
import init, { Renderer as WasmRenderer } from "@takumi-rs/wasm/no-bundler";
import { googleFonts } from "takumi-js/helpers";
import { ImageResponse, type ImageResponseOptions } from "takumi-js/response";

declare const wasm: WebAssembly.Module;
declare const nodes: Parameters<CoreRenderer["prepare"]>[0];
declare const css: CssInput[] | undefined;
declare const style: "normal" | "italic" | "oblique";
declare const fetchImage: (src: string) => Promise<Response>;

// https://github.com/nuxt-modules/og-image/blob/5f191b7dbed829a192cd66fffec7426c07fdc9c7/src/runtime/server/og-image/bindings/takumi/wasm.ts
export const initWasmPromise: Promise<unknown> = init({ module_or_path: wasm });

// https://github.com/nuxt-modules/og-image/blob/d56c0cfe1a433c9dab7e2212e6d9b4f2f8e53b82/src/runtime/server/og-image/takumi/renderer.ts
const fontLoaders = [
  {
    key: "binary",
    name: "Inter",
    data: async () => new Uint8Array(),
    weight: 400,
    style,
    unicodeRange: [[0x20, 0x7f]] satisfies [number, number][],
  },
];

function prepareOptions(signal: AbortSignal) {
  return {
    width: 1200,
    height: 630,
    devicePixelRatio: 1,
    fonts: fontLoaders,
    fontFamilies: ["Inter"],
    css,
    timeMs: 0,
    images: {
      sources: [{ src: "/logo.png", data: new Uint8Array() }],
      fetch: fetchImage,
      allowUrl: (src: string) => src.startsWith("/"),
      timeout: 5000,
      throwOnError: false,
    },
    signal,
  };
}

export async function renderCore(signal: AbortSignal): Promise<Uint8Array[]> {
  const renderer = new CoreRenderer();
  const prepared = await renderer.prepare(nodes, prepareOptions(signal));

  return [
    await renderer.render(prepared, { format: "webp", signal }),
    await renderer.render(prepared, { format: "raw", signal }),
  ];
}

export async function renderWasm(signal: AbortSignal): Promise<Uint8Array[]> {
  const renderer = new WasmRenderer();
  const prepared = await renderer.prepare(nodes, prepareOptions(signal));

  return [
    await renderer.render(prepared, { format: "png", signal }),
    await renderer.render(prepared, { format: "raw", signal }),
  ];
}

// https://github.com/fuma-nama/fumadocs/blob/8364cb7ee14b4fa73f56684de6cf18fee8cd3272/packages/radix-ui/src/og/takumi.tsx
export function generateOGImage(options: { title: string } & ImageResponseOptions): ImageResponse {
  const { title, ...rest } = options;

  return new ImageResponse(<div>{title}</div>, {
    width: 1200,
    height: 630,
    ...rest,
  });
}

// https://github.com/fuma-nama/fumadocs/blob/dc138d208661c637ed8aa7909e9e60902711cc1f/apps/docs/app/og/%5B...slug%5D/generate.tsx
export async function getImageResponseOptions(name?: string): Promise<ImageResponseOptions> {
  const font = await readFile("./JetBrainsMono-Regular.ttf").then((data) => ({
    name: "JetBrainsMono",
    data,
    weight: 400,
  }));
  const fonts = [font];

  return {
    width: 1200,
    height: 630,
    format: "webp",
    fonts: name ? [...fonts, ...(await googleFonts([{ name, weight: [400, 600] }]))] : fonts,
  };
}
