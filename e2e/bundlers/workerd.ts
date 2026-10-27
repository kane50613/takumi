import { Miniflare } from "miniflare";

const [directory, entry] = process.argv.slice(2);
const miniflare = new Miniflare({
  modules: true,
  modulesRoot: directory,
  scriptPath: `${directory}/${entry}`,
  modulesRules: [
    { type: "ESModule", include: ["**/*.js", "**/*.mjs"] },
    { type: "CompiledWasm", include: ["**/*.wasm"] },
  ],
  compatibilityDate: "2025-10-30",
  compatibilityFlags: ["nodejs_compat"],
});

try {
  const response = await miniflare.dispatchFetch("http://e2e/");
  const body = Buffer.from(await response.arrayBuffer());

  console.log(`status ${response.status}`);
  console.log(`magic ${body.subarray(0, 4).toString("hex")}`);

  if (response.status !== 200) {
    console.log(body.toString().slice(0, 300));
  }
} finally {
  await miniflare.dispose();
}
