---
name: takumi-migrate
description: Use when moving image or PDF generation to Takumi from satori, `next/og`, `@vercel/og`, Puppeteer, Playwright, node-html-to-image, or `@react-pdf/renderer`, or when code imports one of them to make an image or PDF.
---

# Migrate to Takumi

A migration is a swap plus a cleanup. Much of the code around the old render works around a limit Takumi does not have, and each such workaround should go. Read the file for the source library before step 2:

| Source                                    | File                         |
| ----------------------------------------- | ---------------------------- |
| satori, `next/og`, `@vercel/og`           | [satori.md](satori.md)       |
| Puppeteer, Playwright, node-html-to-image | [browser.md](browser.md)     |
| `@react-pdf/renderer`                     | [react-pdf.md](react-pdf.md) |

## Steps

1. **Save the old output.** List every template the old library renders, and render each one with the old code and fixed sample data. Done when each template has a saved image or PDF from the old code.

2. **Swap the render.** Follow the swap in the source file. Done when no code that makes images or PDFs imports the old library, and `package.json` no longer lists it.

3. **Move the fonts.** Follow `takumi-fonts`. Done when every font the old code read, fetched, or embedded is a `fonts` entry or part of a `googleFonts` call, and no code parses Google Fonts CSS.

4. **Delete the workarounds.** Remove each one the source file lists. Done when every helper left around the render does something Takumi cannot do.

5. **Compare the output.** Render each template with the same sample data and compare it with the output from step 1. Done when each difference is fixed, or Chromium draws the same markup the way Takumi does. For a difference you cannot explain, follow `takumi-debug`.

For a template served from a route, finish with the production build check in `takumi-deploy`.
