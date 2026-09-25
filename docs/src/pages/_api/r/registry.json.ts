import { buildRegistry } from "~/registry/build";

export const GET = () => Response.json(buildRegistry());

export const getConfig = async () => ({ render: "static" }) as const;
