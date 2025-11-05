import ImageResponse from "@takumi-rs/image-response/wasm";
import DocsTemplateV1 from "@takumi-rs/template/docs-template-v1";
import { type ByteBuf, initSync } from "@takumi-rs/wasm";
import module from "@takumi-rs/wasm/takumi_wasm_bg.wasm";
import geist from "../../../assets/fonts/geist/Geist[wght].woff2";

initSync({ module });

let fetchedResources: Promise<Map<string, ByteBuf>>;
const logoUrl = "https://yeecord.com/img/logo.png";

async function prepareResources() {
  const map = new Map();
  const logo = await fetch(logoUrl).then((r) => r.arrayBuffer());

  map.set(logoUrl, logo);

  return map;
}

export default {
  async fetch(request) {
    fetchedResources ??= prepareResources();

    const { searchParams } = new URL(request.url);

    const name = searchParams.get("name") || "Wizard";

    return new ImageResponse(
      <DocsTemplateV1
        title={`Hello, ${name}`}
        description="This is an example of rendering on Cloudflare Workers!"
        icon={<img tw="w-24 rounded-full" src={logoUrl} alt="Logo" />}
        site="Takumi"
        primaryColor="#F48120"
        primaryTextColor="#fff"
      />,
      {
        fetchedResources: await fetchedResources,
        width: 1200,
        height: 630,
        format: "webp",
        fonts: [geist],
      },
    );
  },
} satisfies ExportedHandler<Env>;
