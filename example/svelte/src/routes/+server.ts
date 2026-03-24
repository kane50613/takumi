import { ImageResponse } from "@takumi-rs/image-response";

export function GET() {
  return new ImageResponse("Hello World", {
    width: 1200,
    height: 630,
    headers: {
      "cache-control": "no-cache, no-store, must-revalidate",
    },
  });
}
