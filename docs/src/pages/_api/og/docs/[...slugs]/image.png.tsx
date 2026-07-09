import type { ImageSource } from "takumi-js";
import { googleFonts } from "takumi-js/helpers";
import wasmModule from "takumi-js/wasm";
import type { ApiContext } from "waku/router";
import { generateOGImage } from "fumadocs-ui/og/takumi";
import { source } from "~/source";
import logo from "../../../../../../public/logo.svg?raw";

const images: ImageSource[] = [
  {
    src: "takumi.svg",
    data: Buffer.from(logo),
  },
];

export async function GET(_: Request, { params }: ApiContext<"/og/docs/[...slugs]/image.png">) {
  const page = source.getPage(params.slugs ?? []);

  if (!page) {
    return new Response(undefined, { status: 404 });
  }

  return generateOGImage({
    title: page.data.title,
    description: page.data.description,
    icon: <img src="takumi.svg" alt="Takumi" style={{ width: "4rem", height: "4rem" }} />,
    primaryColor: "hsla(354, 90%, 54%, 0.3)",
    primaryTextColor: "hsl(354, 90%, 54%)",
    site: "Takumi",
    images,
    fonts: googleFonts([
      {
        name: "Noto Serif",
        weight: [600, 800],
      },
    ]),
    // WASM: static prerender shouldn't depend on the native binding resolving in the build sandbox.
    module: wasmModule,
  });
}

export async function getConfig() {
  return {
    render: "static" as const,
    staticPaths: source.getPages().map((page) => page.slugs ?? []),
  };
}
