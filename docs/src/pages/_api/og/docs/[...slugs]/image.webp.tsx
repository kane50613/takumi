import type { ImageSource } from "takumi-js";
import { ImageResponse } from "takumi-js/response";
import type { ApiContext } from "waku/router";
import DocsTemplate from "../../../../../../../takumi-template/src/templates/docs-template";
import { source } from "~/source";
import logo from "../../../../../../public/logo.svg?raw";

const persistentImages: ImageSource[] = [
  {
    src: "takumi.svg",
    data: Buffer.from(logo),
  },
];

export function GET(_: Request, { params }: ApiContext<"/og/docs/[...slugs]/image.webp">) {
  const page = source.getPage(params.slugs ?? []);

  if (!page) {
    return new Response(undefined, { status: 404 });
  }

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
      persistentImages,
      width: 1200,
      height: 630,
      format: "webp",
    },
  );
}

export async function getConfig() {
  return {
    render: "dynamic" as const,
  };
}
