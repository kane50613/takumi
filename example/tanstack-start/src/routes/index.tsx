import ImageResponse from "@takumi-rs/image-response";
import { createFileRoute } from "@tanstack/react-router";
import { Axe } from "lucide-react";
import DocsTemplate from "../../../../takumi-template/src/templates/docs-template";

export const Route = createFileRoute("/")({
  server: {
    handlers: {
      GET({ request }) {
        const { host } = new URL(request.url);

        return new ImageResponse(
          <DocsTemplate
            title={`Hello from ${host}!`}
            description="If you see this, the TanStack Start example works."
            icon={<Axe color="hsl(354, 90%, 60%)" size={64} />}
            primaryColor="hsla(354, 90%, 54%, 0.3)"
            primaryTextColor="hsl(354, 90%, 60%)"
            site="Takumi"
          />,
          {
            width: 1200,
            height: 630,
            format: "webp",
          },
        );
      },
    },
  },
});
