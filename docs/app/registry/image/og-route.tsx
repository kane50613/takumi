import { ImageResponse } from "takumi-js/response";

import BlogPostTemplate from "@/components/takumi/image/blog-post";

export function GET(request: Request) {
  const { searchParams } = new URL(request.url);

  return new ImageResponse(
    <BlogPostTemplate
      author={searchParams.get("author") ?? "Takumi"}
      category={searchParams.get("category") ?? "Engineering"}
      date={searchParams.get("date") ?? "Today"}
      title={searchParams.get("title") ?? "Hello from Takumi"}
    />,
    { width: 1200, height: 630 },
  );
}
