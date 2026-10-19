# Render Tailwind CSS and UnoCSS stylesheets

Compile Tailwind CSS and UnoCSS from the `className` values in `src/`, then pass each stylesheet to the `css` option of `render()`.

![Tailwind CSS output](./output/tailwind-stylesheets.png)
![UnoCSS output](./output/unocss-stylesheets.png)

## Run

Build the [image example packages](../README.md#image-examples) first. Then render both images:

```bash
cd example/css-library-integration
bun render
```

The script writes each compiled stylesheet and its image to `output/`:

- `tailwind.generated.css` and `tailwind-stylesheets.png`
- `unocss.generated.css` and `unocss-stylesheets.png`
