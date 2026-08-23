import { openApiDocument } from "~/openapi";

export const GET = () => Response.json(openApiDocument);

export const getConfig = async () => ({ render: "static" }) as const;
