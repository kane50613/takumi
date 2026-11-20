import { render as renderSvelte } from "svelte/server";
import { render } from "takumi-pdf";
import Invoice from "#lib/components/Invoice.svelte";

export async function GET() {
  const { head, body } = await renderSvelte(Invoice, {
    props: {
      id: 1042,
      items: [
        { name: "Design", price: 900 },
        { name: "Hosting", price: 390 },
      ],
    },
  });

  const pdf = await render(`${head}${body}`, {
    size: "a4",
    fonts: ["https://takumi.kane.tw/fonts/Geist.woff2"],
  });

  return new Response(pdf, {
    headers: { "content-type": "application/pdf" },
  });
}
