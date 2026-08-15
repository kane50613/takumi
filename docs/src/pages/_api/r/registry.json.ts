import { buildRegistry } from "../../../../app/registry/build";

export const GET = () =>
  Response.json(buildRegistry(), {
    headers: { "Content-Type": "application/json" },
  });

export const getConfig = async () => ({ render: "static" }) as const;
