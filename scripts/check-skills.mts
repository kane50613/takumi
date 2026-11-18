import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const skillsDir = ".agents/skills";
const docsDir = "docs/content/docs";
const docsUrl = /https:\/\/takumi\.kane\.tw\/docs(?:\/([\w/-]+))?\.md/g;
const relativeLink = /\]\((?!https?:)([^)#]+)\)/g;
const skillMention = /`(takumi-[a-z-]+)`/g;

const packageNames = new Set(["takumi-js", "takumi-pdf", "takumi-paint"]);
const skillNames = new Set(readdirSync(skillsDir));
const errors: string[] = [];

function docExists(page: string): boolean {
  return existsSync(join(docsDir, `${page}.mdx`)) || existsSync(join(docsDir, page, "index.mdx"));
}

for (const skill of skillNames) {
  const skillPath = join(skillsDir, skill);
  const entry = readFileSync(join(skillPath, "SKILL.md"), "utf8");

  if (!entry.startsWith(`---\nname: ${skill}\n`)) {
    errors.push(`${skillPath}/SKILL.md: frontmatter name must be ${skill}`);
  }

  for (const file of readdirSync(skillPath).filter((name) => name.endsWith(".md"))) {
    const path = join(skillPath, file);
    const text = readFileSync(path, "utf8");

    for (const [url, page] of text.matchAll(docsUrl)) {
      if (!docExists(page ?? "index")) {
        errors.push(`${path}: ${url} has no page in ${docsDir}`);
      }
    }

    for (const [, target] of text.matchAll(relativeLink)) {
      if (!existsSync(join(skillPath, target))) {
        errors.push(`${path}: links to missing file ${target}`);
      }
    }

    for (const [, name] of text.matchAll(skillMention)) {
      if (!packageNames.has(name) && !skillNames.has(name)) {
        errors.push(`${path}: names a skill that does not exist, ${name}`);
      }
    }
  }
}

if (errors.length > 0) {
  console.error(errors.join("\n"));
  process.exit(1);
}
