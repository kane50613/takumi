import { ImageResponse } from "@takumi-rs/image-response";
import { source } from "~/source";
import DocsTemplate from "../../../takumi-template/src/templates/docs-template";
import logo from "../../public/logo.svg?raw";
import type { Route } from "./+types/og";

export function loader({ params }: Route.LoaderArgs) {
  const slugs = params["*"]
    .split("/")
    .filter((v) => v.length > 0)
    .slice(0, -1);
  const page = source.getPage(slugs);

  if (!page) throw new Response(undefined, { status: 404 });

  return new ImageResponse(
    <DocsTemplate
      title={page.data.title}
      description={page.data.description}
      icon={
        <img
          src="takumi.svg"
          alt="Takumi"
          style={{ width: "4rem", height: "4rem" }}
        />
      }
      primaryColor="hsla(354, 90%, 54%, 0.3)"
      primaryTextColor="hsl(354, 90%, 60%)"
      site="Takumi"
    />,
    {
      persistentImages: [
        {
          src: "takumi.svg",
          data: Buffer.from(logo),
        },
      ],
      width: 1200,
      height: 630,
      format: "webp",
    },
  );
}
