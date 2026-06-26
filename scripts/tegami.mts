import { execFileSync } from "child_process";
import { readFile, writeFile } from "fs/promises";
import path from "path";
import { tegami, type PackageOptions, type TegamiPlugin } from "tegami";
import { createCli } from "tegami/cli";
import { github } from "tegami/plugins/github";

const DEP_FIELDS = [
  "dependencies",
  "devDependencies",
  "peerDependencies",
  "optionalDependencies",
] as const;

// `npm publish` (used for OIDC trusted publishing + provenance) doesn't strip the
// `workspace:` protocol the way `bun`/`pnpm publish` do. Resolve it to concrete
// versions for the duration of each package's publish, then restore the manifest.
function workspaceProtocol(): TegamiPlugin {
  const saved = new Map<string, { file: string; original: string }>();

  return {
    name: "workspace-protocol",
    async willPublish({ pkg }) {
      if (pkg.manager !== "npm") return;

      const file = path.join(pkg.path, "package.json");
      const original = await readFile(file, "utf8");
      const manifest = JSON.parse(original);

      let changed = false;
      for (const field of DEP_FIELDS) {
        const deps = manifest[field];
        if (!deps) continue;
        for (const [name, range] of Object.entries(deps)) {
          if (typeof range !== "string" || !range.startsWith("workspace:")) continue;
          const linked = this.graph.get(`npm:${name}`);
          if (!linked) continue;
          const protocol = range.slice("workspace:".length);
          deps[name] =
            protocol === "^" || protocol === "~" ? `${protocol}${linked.version}` : linked.version;
          changed = true;
        }
      }

      if (!changed) return;

      saved.set(pkg.id, { file, original });
      await writeFile(file, `${JSON.stringify(manifest, null, 2)}\n`);
    },
    async afterPublish({ pkg }) {
      const entry = saved.get(pkg.id);
      if (!entry) return;

      await writeFile(entry.file, entry.original);
      saved.delete(pkg.id);
    },
  };
}

// Tegami publishes a pre-packed tarball, which skips npm's `prepublishOnly` — so
// @takumi-rs/core's napi platform packages never ship and its `optionalDependencies`
// stay empty. Stage and publish them at publish time instead, before the core tarball
// is built, so the platform versions exist and core carries the matching optionalDeps.
function napiPlatformPackages(): TegamiPlugin {
  return {
    name: "napi-platform-packages",
    async willPublish({ pkg }) {
      if (pkg.name !== "@takumi-rs/core") return;

      const run = (cmd: string, args: string[]) =>
        execFileSync(cmd, args, { cwd: pkg.path, stdio: "inherit" });

      run("bun", ["run", "artifacts"]);
      run("bunx", ["napi", "prepublish", "-t", "npm", "--no-gh-release"]);
    },
  };
}

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
  plugins: [
    github({ repo: "kane50613/takumi", cli: { versionPr: { base: "master" } } }),
    napiPlatformPackages(),
    workspaceProtocol(),
  ],
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
