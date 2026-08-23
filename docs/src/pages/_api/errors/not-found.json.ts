import { notFoundError } from "~/openapi";

export const GET = () => Response.json(notFoundError, { status: 404 });

export const getConfig = async () => ({ render: "static" }) as const;
