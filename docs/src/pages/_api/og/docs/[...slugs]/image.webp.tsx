import type { ImageSource } from "takumi-js";
import { googleFontSubsets } from "takumi-js/helpers";
import { ImageResponse } from "takumi-js/response";
import wasmModule from "takumi-js/wasm";
import type { ApiContext } from "waku/router";
import DocsTemplate from "../../../../../../../takumi-template/src/templates/docs-template";
import { source } from "~/source";
import logo from "../../../../../../public/logo.svg?raw";

const images: ImageSource[] = [
  {
    src: "takumi.svg",
    data: Buffer.from(logo),
  },
];

export async function GET(_: Request, { params }: ApiContext<"/og/docs/[...slugs]/image.webp">) {
  const page = source.getPage(params.slugs ?? []);

  if (!page) {
    return new Response(undefined, { status: 404 });
  }

  const fonts = await googleFontSubsets(
    `${page.data.title} ${page.data.description ?? ""} Takumi`,
    ["Geist"],
  );

  return new ImageResponse(
    <DocsTemplate
      title={page.data.title}
      description={page.data.description}
      icon={<img src="takumi.svg" alt="Takumi" style={{ width: "4rem", height: "4rem" }} />}
      primaryColor="hsla(354, 90%, 54%, 0.3)"
      primaryTextColor="hsl(354, 90%, 60%)"
      site="Takumi"
    />,
    {
      images,
      fonts,
      width: 1200,
      height: 630,
      format: "webp",
      // Static build-time generation: use WASM so prerendering doesn't depend on the
      // native napi binding resolving inside the Vercel build sandbox.
      module: wasmModule,
    },
  );
}

export async function getConfig() {
  return {
    render: "static" as const,
    staticPaths: source.getPages().map((page) => page.slugs ?? []),
  };
}
