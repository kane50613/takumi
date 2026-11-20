import { render } from "takumi-pdf";

export async function handle(): Promise<Response> {
  const pdf = await render({
    type: "container",
    style: { width: 200, height: 100, backgroundColor: "#F48120" },
    children: [],
  });

  return new Response(pdf, { headers: { "content-type": "application/pdf" } });
}
