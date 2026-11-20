# @takumi-rs/cli

`takumi` renders an HTML file to an image or a PDF, without a headless browser.

```sh
npx @takumi-rs/cli image card.html -o card.png --width 1200 --height 630
npx @takumi-rs/cli pdf invoice.html -o invoice.pdf
npx @takumi-rs/cli paint card.html -o -   # what the render draws, as text
```

The package runs the prebuilt binary for macOS, Windows, or Linux on x64 and arm64, which an optional dependency installs. The [command line guide](https://takumi.kane.tw/docs/cli) covers every command, and `takumi <command> --help` lists each flag.
