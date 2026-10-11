import adapter from "@sveltejs/adapter-vercel";
import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [
    tailwindcss(),
    sveltekit({
      adapter: adapter({
        runtime: "nodejs24.x",
      }),
      dynamicCompileOptions: ({ filename }) =>
        filename.includes("node_modules") ? undefined : { runes: true },
    }),
  ],
  ssr: {
    external: ["@takumi-rs/core"],
  },
});
