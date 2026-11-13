## takumi-cli@0.1.0

### A `<style media>` element's `@import` rules load

`ParsedHtml::styles` lists each `<style>` element with its text and `media` apart, and `StyleElement::css` wraps the text in the `@media` rule that `ParsedHtml::css` holds. The `takumi` command now loads the `@import` rules of a `<style>` element with a `media` attribute and applies them under that media, as Chrome does. Before, it dropped them.

### `takumi upgrade` finds a Windows install and keeps crates.io installs on crates.io

`takumi upgrade` now recognizes `takumi.exe` in Cargo's install record, so a Windows install no longer reads as one Cargo did not make. On Windows it prints the command to run in PowerShell once takumi exits, since Windows cannot replace a running program, and every command it prints on Windows is quoted for PowerShell. A crates.io install reinstalls with `--registry crates-io`, so a configured default registry cannot take its place. A Git URL's password, and its user name unless the URL uses SSH, no longer appear in the printed command.

### `takumi completions` writes a shell completion script

`takumi completions zsh` writes a script that completes takumi's commands, flags, and their values when you press Tab, and bash, fish, PowerShell, and elvish get one too. `--root` and `--allow-read` complete only directories.

### `--log` prints what a render did

`--log info` prints a timed line to standard error for each stage of a render, and `--log debug` adds each resource loaded, each page and why it ended, the CSS the parser dropped, and each font family or image the render found missing.

### `takumi` warns of unsupported properties, and `--deny-warnings` fails on any warning

A declaration whose property takumi does not support, such as a misspelled one, now prints a warning with its line and column. Properties that cannot change a render, such as `cursor`, stay in `--log debug`. `--deny-warnings` exits with `1` and writes no output when the command raised a warning.

### The `takumi` command line is on crates.io

`cargo install --locked takumi-cli` installs the `takumi` command, which renders an HTML file to a PNG, JPEG, WebP, SVG, or PDF. A document reads only the directories you grant, and never the network.

### `takumi image` warns of the characters no font draws

`takumi image` now prints a warning that names each character no loaded font draws, where it drew an empty box without a word. `uncovered_characters` in `takumi` returns the same list for a node. The command line also lists a file that is not there as `missing` in `--list-resources`, refuses a `--margin` or `--page` number without a unit, and points to `--font`, `--root`, or `takumi pdf` in the errors those fix.

### `takumi paint` writes what a render draws, as text or JSON

`takumi paint card.html -o -` prints one line for each box, text, and image of the document: where it lands, what fills it, and whether it lies off the canvas, has no area, or draws nothing. Elements that `opacity: 0` hides are listed after the outline. `--detail drawables` and `--detail full` add each thing a node draws and the paint order, and `--format json` writes the paint tree takumi-paint returns.
