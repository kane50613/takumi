import { tegami, type PackageOptions } from "tegami";
import { createCli } from "tegami/cli";
import { github } from "tegami/plugins/github";

// v2 beta line; clear this for the stable 2.0.0 release.
const prerelease = "beta";

const groupedNpmPackages = [
  "takumi-js",
  "@takumi-rs/core",
  "@takumi-rs/helpers",
  "@takumi-rs/wasm",
  "@takumi-rs/image-response",
];

const independentCrates = ["takumi-core", "takumi-css", "takumi-raster", "takumi-svg"];

const packages: Record<string, PackageOptions<"takumi">> = {
  "cargo:takumi": { group: "takumi" },
};

for (const name of groupedNpmPackages) {
  packages[`npm:${name}`] = {
    group: "takumi",
    npm: prerelease ? { distTag: prerelease } : undefined,
  };
}

for (const name of independentCrates) {
  packages[`cargo:${name}`] = { prerelease };
}

const paper = tegami({
  plugins: [github({ repo: "kane50613/takumi", cli: { versionPr: { base: "master" } } })],
  groups: {
    takumi: { syncBump: true, syncGitTag: true, prerelease },
  },
  packages,
  ignore: [
    /^(npm:)?(docs|takumi-template|example-.*|ffmpeg-keyframe-animation|ffplay|svelte|waku-ssr)$/,
  ],
  npm: {
    client: "bun",
    updateLockFile: true,
    bumpDep: ({ kind }) => (kind === "dependencies" ? "patch" : false),
  },
  cargo: {
    updateLockFile: true,
    bumpDep: ({ kind }) => (kind === "dependencies" ? "patch" : false),
  },
});

void createCli(paper).parseAsync();
