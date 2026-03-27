import { fetchResources } from "../../../takumi-js/src/helpers";
import { ImageResponse } from "takumi-js/response";
import DocsTemplate from "../../../takumi-template/src/templates/docs-template";

const fetchCache = new Map();
const logoUrl = "https://takumi.kane.tw/logo.svg";

export default {
  async fetch(request) {
    const { pathname, searchParams } = new URL(request.url);

    // stop chrome from requesting favicon.ico
    if (pathname === "/favicon.ico") {
      return new Response(null, { status: 204 });
    }

    const name = searchParams.get("name") || "Wizard";

    const fetchedResources = await fetchResources([logoUrl], {
      cache: fetchCache,
    });

    return new ImageResponse(
      <DocsTemplate
        title={`Hello, ${name}`}
        description="This is an example of rendering on Cloudflare Workers!"
        icon={<img tw="w-16" src={logoUrl} alt="Logo" />}
        site="Takumi"
        primaryColor="#F48120"
        primaryTextColor="#fff"
      />,
      {
        fetchedResources,
        width: 1200,
        height: 630,
        format: "png",
      },
    );
  },
} satisfies ExportedHandler<Env>;
