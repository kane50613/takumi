import { ImageResponse } from "takumi-js/response";
import DocsTemplate from "../../../docs/app/registry/image/docs";

// Shared across requests: dedupes concurrent fetches of the same URL and reuses the bytes.
const imageCache = new Map<string, Promise<ArrayBuffer>>();

export default {
  async fetch(request) {
    const { pathname, searchParams } = new URL(request.url);

    // stop chrome from requesting favicon.ico
    if (pathname === "/favicon.ico") {
      return new Response(null, { status: 204 });
    }

    const name = searchParams.get("name") || "Wizard";

    return new ImageResponse(
      <DocsTemplate
        title={`Hello, ${name}`}
        description="This is an example of rendering on Cloudflare Workers!"
        icon={<img tw="w-16" src="https://takumi.kane.tw/logo.svg" alt="Logo" />}
        site="Takumi"
        primaryColor="#F48120"
        primaryTextColor="#fff"
      />,
      {
        images: {
          fetchCache: imageCache,
        },
        width: 1200,
        height: 630,
      },
    );
  },
} satisfies ExportedHandler<Env>;
