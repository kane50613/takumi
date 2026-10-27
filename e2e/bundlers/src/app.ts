import { render } from "takumi-js";

export async function handle(): Promise<Response> {
  const png = await render(
    {
      type: "container",
      style: { width: 200, height: 100, backgroundColor: "#F48120", color: "white", fontSize: 32 },
      children: [{ type: "text", text: "Hello" }],
    },
    { width: 200, height: 100 },
  );

  return new Response(png, { headers: { "content-type": "image/png" } });
}
