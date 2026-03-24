import { ImageResponse } from "@takumi-rs/image-response";

export function GET() {
  return new ImageResponse(
    {
      key: "root",
      type: "div",
      props: {
        tw: "flex items-center justify-center w-full h-full from-cyan-50 to-sky-100 bg-gradient-to-br font-bold text-4xl text-slate-700",
        children: "SvelteKit + Takumi",
      },
    },
    {
      width: 1200,
      height: 630,
      fonts: [
        {
          name: "Geist",
          data: () =>
            fetch("https://takumi.kane.tw/fonts/Geist.woff2").then((res) => res.arrayBuffer()),
        },
      ],
    },
  );
}
