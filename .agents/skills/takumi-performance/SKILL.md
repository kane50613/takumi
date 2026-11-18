---
name: takumi-performance
description: Use when Takumi renders are slow, either the first render or repeated ones, or when a render server's memory grows with traffic or restarts out of memory. This includes tuning `cacheMaxBytes`, `fontCacheMaxBytes`, `setGlyphCacheMaxBytes`, or `fetchCache`, and profiling a render route.
---

# Takumi performance

Measure before you change a budget. A cache helps only when later renders reuse what it keeps, and [performance.md](https://takumi.kane.tw/docs/performance.md) says what each cache holds and how long.

## Steps

1. **Measure cold and warm renders.** Time the first render in a fresh process, then repeated renders in the same process with representative text, images, formats, and concurrency. For memory, record the process RSS across a few hundred renders. Done when you have the cold time, the warm time, and an RSS series for the slow template.

2. **Move setup out of the request.** Done when each of these holds:
   - Any `Renderer` is created at module level, as step 6 of `takumi-og-image` describes.
   - Every font comes from a module-level `fonts` list.
   - `images.fetchCache`, when set, is bounded, since a plain `Map` keeps every URL it sees.

3. **Size the budget that misses.** `cacheMaxBytes` covers decoded images and parsed stylesheets, `fontCacheMaxBytes` covers fonts that `fonts` lists load, and `setGlyphCacheMaxBytes` covers glyphs. The first two belong to a `Renderer` from `takumi-js/node`. Call `setGlyphCacheMaxBytes` before the first render. Change one budget at a time and measure again. Done when each changed budget has a before and after measurement showing it helps, and the others keep their defaults.

4. **Keep RSS flat on Linux.** When RSS keeps growing while the caches are bounded, try the allocator settings in the "Keep memory flat on Linux servers" section and record the RSS series again. Done when RSS levels off across the run, or you have reported the series to the user with the settings tried.
