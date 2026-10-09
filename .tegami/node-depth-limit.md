---
packages:
  "takumi": patch
  "takumi-pdf": patch
  "takumi-html": minor
---

# Refuse node trees nested deeper than 512 levels

A node tree passed in as data now fails with `nodes nest deeper than 512 levels` past Blink's parser depth, instead of overflowing the stack and aborting the process. `takumi_html::DEFAULT_MAX_DEPTH` is now `takumi_core::layout::node::MAXIMUM_DOM_TREE_DEPTH`; `takumi::DEFAULT_MAX_DEPTH` keeps its name.
