---
name: takumi-fonts
description: Use when a Takumi image or PDF needs font coverage beyond the built-in Latin Geist, such as Google Fonts, brand or variable fonts, CJK, Arabic, Devanagari, or emoji. Typical signs are empty boxes, the wrong font or weight, or an emoji that breaks a render.
---

# Font coverage in Takumi

A render has coverage when every character it can receive draws in a font you chose. Takumi never reads system fonts, and its one built-in font, Geist, covers only Latin at weights 300 to 800. [fonts.md](https://takumi.kane.tw/docs/fonts.md) is the reference for every step below.

## Steps

1. **List what the text needs.** Collect the scripts, weights, and styles the template draws, counting the data it renders as well as its fixed text. A title from a CMS or a user can be in any language. Done when you have a list of script, weight, and style triples and know whether the text can contain emoji.

2. **Choose a source for each family.**
   - Google Fonts load through `googleFonts([...])` from `takumi-js/helpers`. In `takumi-js`, pass the call to `fonts` without `await`. For a PDF, import it from `@takumi-rs/helpers` and spread the awaited result into `fonts`. A render downloads only the subsets its text uses, so listing a CJK family costs nothing for a Latin title.
   - A hosted file is a URL string in `fonts`, such as `"https://example.com/Inter.woff2"`.
   - A render that must work offline or in CI reads the file once at module level and passes the bytes.

   List a variable font once, without `weight`, so it serves its whole axis. A render draws only with the fonts its own `fonts` lists and those from `registerFont`, so every render that needs a family lists it. Done when each triple from step 1 maps to an entry, each variable font appears once, and the `fonts` list is a module-level constant.

3. **Name the families in CSS.** Write each loaded family into the `font-family` stack of the text it draws, in the fallback order described in the "Fallback chain" section. A Tailwind `font-mono` or `font-serif` reaches a font only through its `generic` field. Done when each family name in the CSS matches the name of a loaded font.

4. **Choose the emoji source.** This step applies to `takumi-js`. By default, it fetches each emoji's image from a CDN during the render. The "Failed fetches" section of [images.md](https://takumi.kane.tw/docs/images.md) says what a failed fetch does to the render. To keep the CDN out of the render, load a color emoji font through `fonts` and set `emoji: "from-font"`, as the "COLR / bitmap fonts" section of [images.md](https://takumi.kane.tw/docs/images.md) shows. Done when every template that draws user text sets `emoji` on purpose, and the `emoji` option is the only code that sources emoji.

5. **Check coverage.** Render the template with the longest real strings in every script and weight from step 1, plus an emoji if the text can hold one, and open the image. For a PDF, `takumi-pdf` rejects the render and names each character no font draws. Done when no character shows as an empty box and each script draws in its family. Each `font-weight` the CSS asks for also maps to a loaded weight or a variable font's range.
