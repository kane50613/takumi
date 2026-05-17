export { default } from "~/routes/playground";

export async function getConfig() {
  return {
    render: "static" as const,
  };
}
