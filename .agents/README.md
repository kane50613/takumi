# Takumi Agent Skills

This directory contains customized agent instruction skills for AI coding assistants (such as Google Antigravity, Gemini, Cursor, Copilot, etc.) that developers pair-program with in this codebase.

The skills are structured under the `.agents/skills/` directory and help guide models when writing rendering code, debugging layouts, or migrating from other platforms to Takumi.

---

## Available Skills

1. **[`migrate-from-satori`](skills/migrate-from-satori/SKILL.md)**: Guides for moving from Vercel's `satori`/`next/og` package to Takumi. Highlights performance boosts and API differences (handling of fonts, Tailwind CSS, layouts).
2. **[`migrate-from-puppeteer`](skills/migrate-from-puppeteer/SKILL.md)**: Guides for replacing resource-heavy headless browsers (Puppeteer, Playwright, `node-html-to-image`) with Takumi's fast native rendering call.
3. **[`takumi-usage`](skills/takumi-usage/SKILL.md)**: General usage guidelines, font loading patterns, CSS Grid layouts, Tailwind integrations, SVGs, and animations.

---

## How to Install & Load These Skills

If you want these AI agent skills to be active, choose one of the three integration methods:

### Method 1: Workspace-Level Integration (Recommended / Zero-Install)

By keeping the `.agents/` folder committed to the root of your project's Git repository, **any developer** using an agentic IDE/assistant (e.g. Google Antigravity/Gemini) that opens the codebase will automatically discover, load, and activate the skills.

- **Pros**: Zero-install, config-as-code, automatic synchronization across team members.
- **Triggering**: The assistant automatically reads these instructions when keywords like `satori`, `puppeteer`, `render`, `ImageResponse`, or `takumi` are mentioned.

### Method 2: Global Installation (Across All Projects)

If you want these skills to be active globally on your machine across all your workspaces:

1. Copy the skill folders from `.agents/skills/` into your global customizations directory:
   - MacOS/Linux: `~/.gemini/config/skills/`
   - Windows: `%USERPROFILE%\.gemini\config\skills\`
2. Reload or restart your developer agent.

### Method 3: Referenced via `skills.json`

If you maintain a separate customizations folder, you can register external skills without copying the source files. Create or update a `skills.json` file in your workspace configurations root (e.g. `.agents/skills.json` or `~/.gemini/config/skills.json`):

```json
{
  "entries": [{ "path": "path/to/cloned/takumi/.agents/skills" }]
}
```

Replace the path with the absolute or relative path to where the `takumi` repository resides on your machine.
