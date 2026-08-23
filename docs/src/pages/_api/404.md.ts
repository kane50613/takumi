import { NOT_FOUND_MARKDOWN } from "~/not-found-links";

export const GET = () =>
  new Response(NOT_FOUND_MARKDOWN, {
    status: 404,
    headers: { "Content-Type": "text/markdown; charset=utf-8" },
  });

export const getConfig = async () => ({ render: "static" }) as const;
