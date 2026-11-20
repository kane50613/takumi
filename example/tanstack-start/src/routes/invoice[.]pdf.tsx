import { createFileRoute } from "@tanstack/react-router";
import { render } from "takumi-pdf";
import Invoice from "../components/Invoice";

export const Route = createFileRoute("/invoice.pdf")({
  server: {
    handlers: {
      async GET() {
        const pdf = await render(
          <Invoice
            id={1042}
            items={[
              { name: "Design", price: 900 },
              { name: "Hosting", price: 390 },
            ]}
          />,
          {
            size: "a4",
            fonts: ["https://takumi.kane.tw/fonts/Geist.woff2"],
          },
        );

        return new Response(pdf, {
          headers: { "content-type": "application/pdf" },
        });
      },
    },
  },
});
