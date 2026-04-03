import type { RequestHandler } from "@sveltejs/kit";
import { render } from "svelte/server";
import style from "../app.css?inline";
import ImageResponse from "takumi-js/response";
import OgImage from "$lib/components/OgImage.svelte";

export const GET: RequestHandler = async ({ url }) => {
  const { body, head } = await render(OgImage, {
    props: {
      name: url.searchParams.get("name") ?? "Goo goo gaga",
    },
  });

  return new ImageResponse(`${head}${body}`, {
    width: 1200,
    height: 630,
    stylesheets: [style],
    fonts: [
      {
        name: "Geist",
        data: () =>
          fetch("https://takumi.kane.tw/fonts/Geist.woff2").then((res) => res.arrayBuffer()),
      },
    ],
  });
};
