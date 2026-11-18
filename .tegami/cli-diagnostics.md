---
packages:
  takumi: minor
  takumi-core: minor
  takumi-cli: patch
---

# `takumi image` warns of the characters no font draws

`takumi image` used to draw an empty box for a character no loaded font draws, without a word. It now prints a warning that names each one.

```sh
$ takumi image hello.html -o hello.png
warning: no font draws 你 (U+4F60), 好 (U+597D), so each shows as an empty box. Pass a font that does with `--font`, or check the `font-family` of that text.
```

The rest of the command line says more when something is off:

- `--list-resources` lists a file that is not there as `missing`, where it said `blocked`. Scripts that read the status column see the new value.
- `--margin` and `--page` refuse a number without a unit, such as `--margin 5`.
- An error that `--font`, `--root`, or `takumi pdf` fixes now names that flag or command, such as "`takumi image` writes images, but the output ends in `.pdf`. Use `takumi pdf`."

```diff
  $ takumi image page.html --list-resources
- image	blocked	file:///site/gone.png	…
+ image	missing	file:///site/gone.png	…
```

In the `takumi` crate, `uncovered_characters` returns the characters no registered font draws for a render, each once.

```rust
let missing: Vec<char> = takumi::uncovered_characters(options)?;

if !missing.is_empty() {
  eprintln!("no font draws {missing:?}");
}
```
