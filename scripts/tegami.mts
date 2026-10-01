import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";
import { $, YAML } from "bun";
import {
  tegami,
  type BumpType,
  type PackageOptions,
  type TegamiPlugin,
  type WorkspacePackage,
} from "tegami";
import { createCli } from "tegami/cli";
import { cargo } from "tegami/plugins/cargo";
import { github } from "tegami/plugins/github";

// tegami serializes changelogs/manifests in a style oxfmt rejects; reformat
// before the github plugin stages and commits the version branch.
const oxfmt: TegamiPlugin = {
  name: "oxfmt",
  enforce: "pre",
  async applyCliDraft() {
    await $`oxfmt --write .`.quiet();
  },
};

// tegami silently skips a changelog whose frontmatter has no `packages` map.
const requirePackages: TegamiPlugin = {
  name: "require-packages",
  async init() {
    const files = (await readdir(this.changelogDir)).filter((file) => file.endsWith(".md"));
    const unlisted: string[] = [];

    for (const file of files) {
      const content = await readFile(join(this.changelogDir, file), "utf8");
      const frontmatter = /^---\r?\n(.+?)\r?\n---/s.exec(content)?.[1];
      const data: unknown = frontmatter ? YAML.parse(frontmatter) : null;
      const packages = data instanceof Object && "packages" in data ? data.packages : null;

      if (!(packages instanceof Object) || Object.keys(packages).length === 0) unlisted.push(file);
    }

    if (unlisted.length > 0) {
      throw new Error(`List the bumped packages under \`packages\` in ${unlisted.join(", ")}`);
    }
  },
};

const refreshLockfile: TegamiPlugin = {
  name: "refresh-lockfile",
  async applyCliDraft() {
    await $`bun install`.quiet();
    await $`cargo update --workspace`.quiet();
  },
};

type ReleaseGroup = "takumi" | "takumi-pdf" | "takumi-paint";

const releaseGroups: [ReleaseGroup, string[]][] = [
  [
    "takumi",
    [
      "npm:takumi-js",
      "npm:@takumi-rs/core",
      "npm:@takumi-rs/helpers",
      "npm:@takumi-rs/wasm",
      "npm:@takumi-rs/image-response",
      "cargo:takumi",
    ],
  ],
  // The crate is never published; it carries a version so the `/Producer` it
  // writes matches the npm package a reader installed.
  ["takumi-pdf", ["npm:takumi-pdf", "cargo:takumi-pdf"]],
  ["takumi-paint", ["npm:takumi-paint"]],
];

const packages = Object.fromEntries(
  releaseGroups.flatMap(([group, names]) =>
    names.map((name): [string, PackageOptions<ReleaseGroup>] => [name, { group }]),
  ),
);

// Skip versionless dependents (private examples, docs, templates); only real
// `dependencies` bumps propagate.
const bumpDep = ({
  dependent,
  kind,
}: {
  dependent: WorkspacePackage;
  kind: string;
}): BumpType | false => (dependent.version && kind === "dependencies" ? "patch" : false);

const paper = tegami({
  plugins: [
    requirePackages,
    oxfmt,
    refreshLockfile,
    github({ repo: "kane50613/takumi", versionPr: { base: "master" } }),
    cargo({ updateLockFile: true, bumpDep }),
  ],
  groups: {
    takumi: { syncBump: true, syncGitTag: true },
    "takumi-pdf": { syncBump: true },
    "takumi-paint": { syncBump: true },
  },
  packages,
  npm: { client: "bun", updateLockFile: true, bumpDep },
});

void createCli(paper).parseAsync();
