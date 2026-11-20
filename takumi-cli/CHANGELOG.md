## takumi-cli@0.2.0

### `takumi` errors say how to fix more mistakes

- An image too tall for the canvas names the most height the width and `--dpr` allow.
- An HTML file that holds binary data says it is not HTML, where it suggested converting its encoding. A UTF-16 file is named as UTF-16, with an `iconv` command that converts it.
- `--page` and `--margin` values takumi does not read come with examples of the values it does.
- `--selector` names an inline element without a background or border only when one matches, and otherwise says no element matches.

```diff
  $ takumi image tall.html -o tall.png
- error: the image would be 0px tall or over 64 megapixels at the content's height. Pass `--height`.
+ error: the image would be 0px tall or over 64 megapixels at the content's height, which at 1200px wide and `--dpr 1` allows at most 55924px. Pass `--height`.
```

### `takumi paint` keeps the break between wrapped lines

A text that wrapped onto several lines used to print with its lines run together, so `Takumi CLI, tested by an agent` wrapped after `tested` printed as `"Takumi CLI, testedby an agent"`. Each line break now prints as `\n`.

```diff
- text p 8,8 284x82 "Takumi CLI, testedby an agent" Geist 32px #000000
+ text p 8,8 284x82 "Takumi CLI, tested\nby an agent" Geist 32px #000000
```

### `takumi` names standard input, `--font`, and a missing directory in what it prints

`--log info` names standard input for `-`, where it printed the URL of the working directory. A `--font` file that is missing or not a font fails with an error that starts with the flag. An output in a directory that does not exist names that directory.

```diff
  $ cat card.html | takumi image - -o card.png --log info
- [     0.0ms] input: read file:///home/me/site/, 198 bytes
+ [     0.0ms] input: read standard input, 198 bytes

  $ takumi image card.html -o card.png --font nope.ttf
- error: nope.ttf: No such file or directory (os error 2)
+ error: `--font nope.ttf`: No such file or directory (os error 2)

  $ takumi image card.html -o out/card.png
- error: out/card.png: No such file or directory (os error 2)
+ error: out/card.png: the directory out does not exist. Create it before writing there.
```

`takumi image --help` and `takumi pdf --help` also name the tab-separated columns `--list-resources` prints.

### `@media (prefers-color-scheme)` matches, and `takumi --color-scheme dark` renders the dark variant

`prefers-color-scheme` used to match nothing. It now matches `light` by default, and `dark` when the viewport asks for it, so a page that keeps its dark colors in a `prefers-color-scheme: dark` rule renders them.

```sh
takumi image email.html -o email-dark.png --width 600 --color-scheme dark
```

```rust
let viewport = Viewport::new((600, 400)).with_color_scheme(ColorScheme::Dark);
```

- `takumi image` and `takumi paint` take `--color-scheme light` or `dark`.
- In Rust, `Viewport::with_color_scheme` sets it, and `ColorScheme` is in the prelude. `Viewport` gains the `color_scheme` field, so a `Viewport { .. }` literal needs it. `Viewport::new` and its `with_*` methods need no change.
- A rule under `prefers-color-scheme: light` now applies by default, as in a browser.
- `light-dark()` still takes its first color.

### Migration

A `Viewport` struct literal names the new field, unless it copies the rest with `..`:

```diff
  let viewport = Viewport {
    size: (1200, 630).into(),
    font_size: 16.0,
    device_pixel_ratio: 1.0,
    media_target: MediaTarget::Screen,
    unit_reference: None,
+   color_scheme: ColorScheme::Light,
  };
```

### Text fields and `--list-resources` no longer warn of a missing font

A page with a text `<input>` warned that no face is loaded for `Arial`, since the default style of a text field named it, so `--deny-warnings` failed on any page with one. The default style now names `system-ui`, which draws in the same font when no Arial is loaded. `takumi --list-resources` no longer warns that an `@font-face` family is missing either.

```diff
  $ takumi image signup.html -o signup.png --deny-warnings
- warning: no face is loaded for font-family "Arial", so text falls back to the next family that has one. Load the font, or check the name in font-family.
```

### `cargo binstall takumi-cli` installs a prebuilt binary

Each takumi-cli release on GitHub now carries a binary for macOS, Windows, and Linux, each on x64 and arm64, so `cargo binstall` installs takumi in seconds instead of building it. The Linux binary is statically linked and runs on both glibc and musl.

```sh
cargo binstall takumi-cli
```

### `takumi paint` marks a box the canvas cuts off

A node that crosses an edge of the canvas now ends with `cut off at the <side> edge`, or names each edge it crosses. Before, only a node wholly past an edge was marked, as `offscreen`.

```diff
- box div#cta 1100,600 200x60 background #4338ca
+ box div#cta 1100,600 200x60 background #4338ca cut off at the right and bottom edges
```

### `takumi image --help` describes `--selector` and `--color-scheme` as they behave

`--selector` now says it takes a lone class such as `.card`. `--color-scheme` now says `light-dark()` always takes its first color, whatever the scheme.

### `takumi image --selector` renders one element at its own size

`--selector` keeps only the first element it matches and crops the image to that element's box, so a card or a slide renders without the page around it.

```sh
takumi image page.html -o pricing.png --selector .pricing-card
```

- It takes a type, `#id`, and `.class` written together, such as `section.pricing-card`.
- It writes PNG, JPEG, or WebP. An SVG output with `--selector` exits with code `2`.
- No match fails with `no element matches`.
- In Rust, `Bitmap::crop` is now public, and returns `None` for an empty region or one that does not fit the bitmap.

```rust
let bitmap = Bitmap::from_raw(320, 256, vec![0; 320 * 256 * 4]).ok_or("invalid bitmap data")?;
let card = bitmap.crop(40, 120, 280, 136).ok_or("the card lies outside the image")?;
```

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
