export { default } from "~/routes/showcase";

export async function getConfig() {
  return {
    render: "static" as const,
  };
}
