---
name: takumi-debug
description: Find why a Takumi render differs from a browser or from what the template intends, such as a missing or misplaced element, an ignored style, a wrong color, or clipped text. Use when the output looks wrong and the cause is not a missing font or a load error.
---

# Debug a Takumi render

Read the paint tree before you change the template. It lists every box, text run, and image with its rectangle on the canvas, so it shows where an element went without guessing from pixels.

## Steps

1. **Reproduce at a fixed size.** Build one input that shows the difference, with the same `width`, `height`, `css`, and `fonts` the real render uses. Render it to a file and open the image. Done when you have that input and an image that shows the problem.

2. **Read the paint tree.** For JSX, install `takumi-paint` as a dev dependency and print each node:

   ```tsx
   import { paint } from "takumi-paint";

   const tree = await paint(<Card />, { width: 1200, height: 630 });

   for (const node of tree.nodes) {
     console.log(node.type, node.element?.tagName, node.element?.className, node.bounds);
   }
   ```

   `paint` takes image bytes, not URLs, so a remote image paints as missing here. For an HTML file, the `takumi` CLI prints the tree with `takumi paint card.html -o - --width 1200 --height 630`, and [cli/paint.md](https://takumi.kane.tw/docs/cli/paint.md) explains its markers for offscreen, empty, and unpainted nodes. Done when you can name the wrong node and point at the line or field that shows it.

3. **Check each declaration on that node.** Look up every property it sets in [reference.md](https://takumi.kane.tw/docs/reference.md). The "Strict style objects" section of [concepts.md](https://takumi.kane.tw/docs/concepts.md) says what happens to a property outside that list and to a value that does not parse, and the rest of "Where it differs from a browser" covers pseudo-classes, `@media`, and image URLs inside `css`. Done when every declaration on the node is supported with the value it uses, or you have found the one that is not.

4. **Compare with Chromium.** Open the same markup in Chromium at the same viewport. Done when the difference falls in one of these cases:
   - Chromium draws the same thing, so the template is wrong. Fix the template.
   - The declaration is unsupported. Replace it with a supported one and check the image again.
   - Chromium differs and every declaration is supported. Draft an issue for the user with the minimal input, both images, and the paint tree.
