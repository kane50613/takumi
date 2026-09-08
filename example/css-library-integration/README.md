# Render Tailwind CSS and UnoCSS stylesheets

Compile Tailwind CSS and UnoCSS in the same process, then pass each stylesheet to Takumi's `css` render option.

![Tailwind CSS output](./output/tailwind-stylesheets.png)
![UnoCSS output](./output/unocss-stylesheets.png)

## Run

Before running the example, build the native package once from the workspace root.

```bash
bun --filter '*' run build
```

Then render both example images:

```bash
cd example/css-library-integration
bun render
```

This writes:

- `output/tailwind.generated.css`
- `output/tailwind-stylesheets.png`
- `output/unocss.generated.css`
- `output/unocss-stylesheets.png`
