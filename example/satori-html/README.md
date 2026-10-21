# Render satori-html output with Takumi

Convert an HTML template with `satori-html`, then render the result as a PNG with `takumi-js`.

`render()` also accepts an HTML string directly. This example is for projects that already build templates with `satori-html`.

## Run

Build the [image example packages](../README.md#image-examples) first. Then run the script with Node.js:

```bash
cd example/satori-html
node index.ts
```

Open `output.png` to see the result.

Use a Node.js version that runs TypeScript files directly, such as 22.18 or newer. Bun crashes inside `ultrahtml`, the HTML parser `satori-html` uses ([ultrahtml#66](https://github.com/natemoo-re/ultrahtml/issues/66)).
