## takumi-cli@0.1.0

### The paint tree lists the CSS a render dropped

A declaration or at-rule that takumi drops no longer vanishes without a trace. The paint tree lists each one once, with its property and value as written, why takumi dropped it, and where it was written. The image stays the same.

```ts
const tree = await paint(html, { width: 600, height: 200 });

console.log(tree.ignoredDeclarations);
// [{ property: "writing-mode", value: "vertical-rl", reason: "unknown-property", origin: "style-attribute" }]
```

- `reason` is `unknown-property`, `invalid-value`, or `unknown-at-rule`.
- `origin` is `style-attribute`, `style-sheet`, or `style-object`.
- `takumi paint` prints the list under `ignored`, and its JSON output holds it as `ignoredDeclarations`.
- `needs()` in `@takumi-rs/core`, `@takumi-rs/wasm`, and `takumi-pdf` returns the same list as `ignoredDeclarations`.
- In Rust, `RenderNeeds::ignored_declarations` and `PaintDocument::ignored_declarations` hold the same list, and `StyleSheet::ignored_declarations` holds what one style sheet dropped.

### `takumi completions` writes a shell completion script

`takumi completions <shell>` writes a script that completes takumi's commands, flags, and their values when you press Tab. It supports `bash`, `zsh`, `fish`, `powershell`, and `elvish`. In bash, zsh, and fish, `--root` and `--allow-read` complete only directories.

```sh
mkdir -p ~/.zfunc
takumi completions zsh > ~/.zfunc/_takumi
# Then add `fpath+=~/.zfunc` before `compinit` in ~/.zshrc
```

`takumi completions --help` lists where each shell reads the script.

### The `takumi` command line is on crates.io

`takumi` renders an HTML file to a PNG, JPEG, WebP, SVG, or PDF from the command line. A document reads only the directories you grant, and never the network.

```sh
cargo install --locked takumi-cli
takumi image card.html -o card.png
takumi pdf report.html -o report.pdf
```

### `takumi paint` writes what a render draws, as text or JSON

`takumi paint` prints one line for each box, text, and image of a document, with where it lands and what fills it. A line also says when its node lies off the canvas, has no area, or draws nothing, and the elements that `opacity: 0` hides follow the outline.

```sh
$ takumi paint card.html -o - --width 300 --height 100
canvas 300x100
box html 0,0 300x121
  box body 0,0 300x121
    box div#card 0,0 300x100 background #0f172a clips-overflow
      box h1.title.big 0,0 300x41
        text h1.title.big 0,0 300x41 "Hello" Geist 700 32px #ffffff
      box div.ghost 900,0 20x20 background #ff0000 offscreen past the right edge
not painted
  section opacity: 0
```

- `--detail drawables` adds each thing a node draws.
- `--detail full` adds the paint order as well, and in JSON every glyph.
- `--format json`, or an output that ends in `.json`, writes the paint tree that takumi-paint returns.

### `takumi image` warns of the characters no font draws

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

### `takumi pdf` takes a whole HTML file as a header or footer

A `--header` or `--footer` file that opens with a doctype or `<html>` now fills the band with its body's content, as Chromium does with a header template. Before, the file brought its body's margin into the band.

- A page and its header that both write `<img src="logo.png">` for different files now each draw their own image.
- `takumi pdf --pages 1,-` is refused with "`-` names no page. Write a page on at least one side of `-`."
- An empty `url()` in a style sheet loads nothing.
- `takumi image --dpr` with an `.svg` output is an error, since an SVG has no pixels to scale.

In Rust, `FromHtmlOptions::fragment` parses a source as a fragment even when it opens with a doctype or `<html>`, dropping its `html`, `head`, and `body` tags.

```rust
let options = FromHtmlOptions::builder().fragment(true).build();
// The node is the <p> alone
let node = parse_html("<!doctype html><html><body><p>hi</p></body></html>", options)?.node;
```

### `takumi` loads the `@import` rules of a `<style media>` element

The `takumi` command used to drop the `@import` rules of a `<style>` element with a `media` attribute. It now loads them and applies them under that media, as Chrome does.

```html
<style media="print">
  @import "print.css";
</style>
```

- `ParsedHtml::styles` lists each non-empty CSS `<style>` element in the HTML namespace that applies, so not one inside an inline `<svg>`, with its `text` and `media` apart. That is every untitled one, plus the titled ones in the preferred style sheet set.
- `StyleElement::css` returns the text inside the `@media` rule that `ParsedHtml::css` holds.

### `takumi` warns of unsupported properties, and `--deny-warnings` fails on any warning

A declaration whose property takumi does not support, such as a misspelled one, now prints a warning with its line and column. `--deny-warnings` turns any warning into a failure that exits with `1` and writes no output.

```sh
$ takumi image card.html -o card.png --deny-warnings
warning: ignored the property `widht` at line 1, column 7 of its style sheet or `style` attribute, since takumi does not support it. Check its spelling.
error: `--deny-warnings` is set and the command raised 1 warning, so it wrote nothing. Fix each one, or drop the flag to write the output anyway.
```

Properties that cannot change a render, such as `cursor`, raise no warning and show only under `--log debug`.

### `--log` prints what a render did

`--log info` prints a line to standard error for each stage of a render, with the time since the command began.

```sh
$ takumi image card.html -o card.png --log info
[     0.2ms] input: read file:///site/card.html, 198 bytes
[     1.4ms] stylesheet: style sheets loaded: 0
[     3.6ms] resource: images loaded: 0
[     8.2ms] render: rendered 1464 bytes
[     8.2ms] output: writing card.png
```

`--log debug` adds a line for:

- each resource loaded
- each page, and why it ended
- the CSS the parser dropped
- each font family or image the render found missing

### `takumi upgrade` finds a Windows install and keeps a crates.io install on crates.io

On Windows, `takumi upgrade` now recognizes a takumi that `cargo install` put there, and gives you the command to finish the upgrade yourself. A takumi installed from crates.io reinstalls from crates.io, even when Cargo has another default registry.

```sh
$ takumi upgrade --dry-run
# Before
cargo install --locked takumi-cli --root /opt/takumi
# After
cargo install --locked --registry crates-io takumi-cli --root /opt/takumi
```

- On Windows, `takumi upgrade` no longer says takumi was not installed with `cargo install`.
- On Windows, it does not upgrade takumi itself, since Windows cannot replace a running program. It fails with "Windows cannot replace takumi while it runs. Run `cargo install …` in PowerShell once this command exits." Every command it prints there is quoted for PowerShell.
- A printed command leaves out the password in a Git URL, and the user name too unless the URL uses SSH.
