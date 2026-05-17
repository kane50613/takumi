export { default } from "~/routes/home";

export async function getConfig() {
  return {
    render: "static" as const,
  };
}
