---
packages:
  takumi-core: minor
  takumi-html: minor
---

# Draw the value of text fields such as `<input type="text">`

An `<input>` of type `text`, `search`, `tel`, `url`, `email`, `password`, or `number` now draws its `value` inside the field, as Chromium does. The field takes Chromium's default font, border, and padding, sizes itself from the `size` attribute, centers its text vertically, and sits on the baseline of its line. A password shows a dot for each character, a number field with an invalid value shows nothing, and a disabled field turns gray. `<input type="hidden">` takes no space. These inputs used to draw nothing, not even their author-set width.
