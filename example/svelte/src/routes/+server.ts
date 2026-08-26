import { render } from "svelte/server";
import style from "../app.css?inline";
import ImageResponse from "takumi-js/response";
import OgImage from "$lib/components/OgImage.svelte";
import type { RequestEvent } from "./$types";

export async function GET({ url }: RequestEvent) {
  const { body, head } = await render(OgImage, {
    props: {
      name: url.searchParams.get("name") ?? "Goo goo gaga",
    },
  });

  return new ImageResponse(`${head}${body}`, {
    width: 1200,
    height: 630,
    css: style,
    fonts: ["https://takumi.kane.tw/fonts/Geist.woff2"],
  });
}
