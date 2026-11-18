---
packages:
  takumi: minor
  takumi-raster: minor
---

# `write_animation_frames` writes rendered frames in any animation format

`write_animation_frames` takes the frames `render_animation` returned and an `AnimationFormat`, which already carries each format's options. It replaces `write_animated_gif`, `write_animated_png`, and `write_animated_webp`, which are now deprecated and still work.

```diff
- write_animated_gif(Cow::Borrowed(&frames), &mut out, AnimatedGifOptions::default())?;
+ write_animation_frames(
+   Cow::Borrowed(&frames),
+   AnimationFormat::Gif(AnimatedGifOptions::default()),
+   &mut out,
+ )?;
```

`takumi-raster` no longer re-exports `MeasuredNode`, `MeasuredTextRun`, and `RenderNeeds`. Import them from `takumi::prelude` or `takumi_core::layout`. `takumi::prelude` still exports all three.
